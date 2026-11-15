use std::cmp::Reverse;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use crate::wip::job::id::RunId;
use crate::wip::kernel::child::alive;
use crate::wip::layout::KEY_FILE;
use crate::wip::layout::RUNNING_FILE;
use crate::wip::layout::RUNS_DIR;
use crate::wip::layout::STORE_DIR;

// Keeps the newest runs and every run still going, deletes the rest, and
// drops from the store every product no kept run refers to. Only folders
// named as a run id are touched.
pub fn sweep(root: &Path, keep: usize) {
    let Ok(entries) = fs::read_dir(root.join(RUNS_DIR)) else {
        return;
    };
    let mut runs: Vec<(RunId, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            Some((RunId::parse(&name)?, entry.path()))
        })
        .collect();
    runs.sort_by_key(|run| Reverse(run.0));
    let (kept, beyond) = runs.split_at(keep.min(runs.len()));
    let mut referenced: HashSet<String> = HashSet::new();
    for (_, dir) in kept {
        keys_under(dir, &mut referenced);
    }
    for (_, dir) in beyond {
        if running(dir) {
            keys_under(dir, &mut referenced);
        } else {
            let _ = fs::remove_dir_all(dir);
        }
    }
    if let Ok(entries) = fs::read_dir(root.join(STORE_DIR)) {
        for entry in entries.flatten() {
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            if !referenced.contains(&name) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

fn running(dir: &Path) -> bool {
    fs::read_to_string(dir.join(RUNNING_FILE))
        .ok()
        .and_then(|pid| pid.trim().parse::<u32>().ok())
        .is_some_and(alive)
}

fn keys_under(dir: &Path, into: &mut HashSet<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            keys_under(&path, into);
        } else if entry.file_name() == KEY_FILE {
            if let Ok(key) = fs::read_to_string(&path) {
                into.insert(key.trim().to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use std::process;

    use super::sweep;
    use crate::wip::job::id::RunId;
    use crate::wip::layout::KEY_FILE;
    use crate::wip::layout::RUNNING_FILE;
    use crate::wip::layout::RUNS_DIR;
    use crate::wip::layout::STORE_DIR;

    fn root(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("arbiter-sweep-{name}-{}", RunId::next()));
        fs::create_dir_all(dir.join(RUNS_DIR)).expect("a root is made");
        fs::create_dir_all(dir.join(STORE_DIR)).expect("a store is made");
        dir
    }

    fn run_dir(root: &Path, run: RunId, keys: &[&str]) -> PathBuf {
        let dir = root.join(RUNS_DIR).join(run.to_string());
        for (n, key) in keys.iter().enumerate() {
            let attempt = dir.join("gate").join(format!("task{n}#1"));
            fs::create_dir_all(&attempt).expect("an attempt folder is made");
            fs::write(attempt.join(KEY_FILE), key).expect("a key is written");
        }
        fs::create_dir_all(&dir).expect("a run folder is made");
        dir
    }

    fn stored(root: &Path, key: &str) {
        fs::write(root.join(STORE_DIR).join(key), b"{}").expect("a product is stored");
    }

    #[test]
    fn a_sweep_keeps_the_newest_runs_and_the_products_they_refer_to_and_drops_the_rest() {
        let root = root("sweep");
        let runs: Vec<RunId> = (0..3).map(|_| RunId::next()).collect();
        let oldest = run_dir(&root, runs[0], &["aaaa"]);
        let middle = run_dir(&root, runs[1], &["bbbb"]);
        let newest = run_dir(&root, runs[2], &["cccc", "bbbb"]);
        for key in ["aaaa", "bbbb", "cccc", "dddd"] {
            stored(&root, key);
        }
        fs::create_dir_all(root.join(RUNS_DIR).join("notes")).expect("a stray folder");
        sweep(&root, 2);
        assert!(!oldest.exists(), "the run beyond the kept ones is gone");
        assert!(middle.exists() && newest.exists());
        assert!(
            root.join(RUNS_DIR).join("notes").exists(),
            "only run folders are touched"
        );
        let left: Vec<String> = {
            let mut names: Vec<String> = fs::read_dir(root.join(STORE_DIR))
                .expect("the store is read")
                .flatten()
                .filter_map(|entry| entry.file_name().into_string().ok())
                .collect();
            names.sort();
            names
        };
        assert_eq!(left, ["bbbb", "cccc"]);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_sweep_leaves_a_run_still_going_whatever_its_age() {
        let root = root("going");
        let runs: Vec<RunId> = (0..2).map(|_| RunId::next()).collect();
        let going = run_dir(&root, runs[0], &["aaaa"]);
        fs::write(going.join(RUNNING_FILE), process::id().to_string()).expect("marked");
        let stale = run_dir(&root, runs[1], &[]);
        fs::write(stale.join(RUNNING_FILE), "4000000000").expect("marked");
        stored(&root, "aaaa");
        sweep(&root, 0);
        assert!(going.exists(), "a run whose process is alive is kept");
        assert!(
            !stale.exists(),
            "a marker from a process that is gone keeps nothing"
        );
        assert!(root.join(STORE_DIR).join("aaaa").exists());
        let _ = fs::remove_dir_all(&root);
    }
}
