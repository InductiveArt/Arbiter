use std::cmp::Reverse;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use crate::contract::RunId;
use crate::contract::TaskAttempt;
use crate::execution::run_folder::CACHE_DIR;
use crate::execution::run_folder::OUTPUTS_DIR;
use crate::execution::run_folder::RUNNING_FILE;
use crate::execution::run_folder::RUNS_DIR;
use crate::kernel::child::alive;
use crate::protocol::attempt_folder::RECORD_FILE;
use crate::value::task_cache::TaskCache;
use crate::value::task_cache_key::TaskCacheKey;
use crate::value::task_output_hash::TaskOutputHash;
use crate::value::task_output_store::TaskOutputStore;

// Keeps the newest runs and every run still going, deletes the rest, and
// drops every kept output and every cache entry that no attempt record of a
// kept run names. Only folders named as a run id are touched.
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
    let mut named = Named::default();
    for (_, dir) in kept {
        named.under(dir);
    }
    for (_, dir) in beyond {
        if running(dir) {
            named.under(dir);
        } else {
            let _ = fs::remove_dir_all(dir);
        }
    }
    TaskOutputStore::at(root.join(OUTPUTS_DIR)).prune(&named.outputs);
    TaskCache::at(root.join(CACHE_DIR)).prune(&named.keys);
}

fn running(dir: &Path) -> bool {
    fs::read_to_string(dir.join(RUNNING_FILE))
        .ok()
        .and_then(|pid| pid.trim().parse::<u32>().ok())
        .is_some_and(alive)
}

// The outputs and the cache keys the attempt records name.
#[derive(Default)]
struct Named {
    outputs: HashSet<TaskOutputHash>,
    keys: HashSet<TaskCacheKey>,
}

impl Named {
    // Every attempt record under the folder, at any depth; a record that does
    // not read names nothing.
    fn under(&mut self, dir: &Path) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                self.under(&path);
            } else if entry.file_name() == RECORD_FILE {
                let attempt = TaskAttempt::read(&path).ok();
                if let Some(attempt) = attempt {
                    self.outputs.extend(attempt.output_hash());
                    self.keys.extend(attempt.task_cache_key());
                }
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
    use crate::contract::RunId;
    use crate::contract::RunRecord;
    use crate::contract::TaskAttempt;
    use crate::contract::TaskAttemptId;
    use crate::execution::run_folder::CACHE_DIR;
    use crate::execution::run_folder::OUTPUTS_DIR;
    use crate::execution::run_folder::RUNNING_FILE;
    use crate::execution::run_folder::RUNS_DIR;
    use crate::protocol::attempt_folder::RECORD_FILE;
    use crate::value::task_cache::TaskCache;
    use crate::value::task_cache_key::TaskCacheKey;
    use crate::value::task_output_hash::TaskOutputHash;

    fn root(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("task-runner-sweep-{name}-{}", RunId::next()));
        fs::create_dir_all(dir.join(RUNS_DIR)).expect("a root is made");
        dir
    }

    // A run whose attempts each left the output named, cached under its key.
    fn run_dir(root: &Path, run: RunId, left: &[&str]) -> PathBuf {
        let dir = root.join(RUNS_DIR).join(run.to_string());
        fs::create_dir_all(&dir).expect("a run folder is made");
        for (n, name) in left.iter().enumerate() {
            let folder = dir.join("gate").join(format!("task{n}#1"));
            fs::create_dir_all(&folder).expect("an attempt folder is made");
            let mut attempt = TaskAttempt::unrun(
                TaskAttemptId::new(run, format!("gate/task{n}"), 1),
                RunRecord::passed(),
            );
            attempt.output = Some(output(name).as_str().to_string());
            attempt.cache_key = Some(key(name).as_str().to_string());
            attempt
                .write(&folder.join(RECORD_FILE))
                .expect("a record is written");
        }
        dir
    }

    fn output(name: &str) -> TaskOutputHash {
        TaskOutputHash::of(name.as_bytes())
    }

    fn key(name: &str) -> TaskCacheKey {
        TaskCacheKey::of(&[Some(name.as_bytes())])
    }

    fn kept(root: &Path, name: &str) {
        fs::create_dir_all(root.join(OUTPUTS_DIR)).expect("a store is made");
        fs::write(root.join(OUTPUTS_DIR).join(output(name).as_str()), name)
            .expect("an output is kept");
        TaskCache::at(root.join(CACHE_DIR))
            .put(&key(name), &output(name))
            .expect("an entry is written");
    }

    fn left(root: &Path, folder: &str) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(root.join(folder))
            .expect("the folder is read")
            .flatten()
            .filter_map(|entry| entry.file_name().into_string().ok())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn a_sweep_keeps_the_newest_runs_and_what_their_records_name_and_drops_the_rest() {
        let root = root("sweep");
        let runs: Vec<RunId> = (0..3).map(|_| RunId::next()).collect();
        let oldest = run_dir(&root, runs[0], &["aaaa"]);
        let middle = run_dir(&root, runs[1], &["bbbb"]);
        let newest = run_dir(&root, runs[2], &["cccc", "bbbb"]);
        for name in ["aaaa", "bbbb", "cccc", "dddd"] {
            kept(&root, name);
        }
        fs::create_dir_all(root.join(RUNS_DIR).join("notes")).expect("a stray folder");
        sweep(&root, 2);
        assert!(!oldest.exists(), "the run beyond the kept ones is gone");
        assert!(middle.exists() && newest.exists());
        assert!(
            root.join(RUNS_DIR).join("notes").exists(),
            "only run folders are touched"
        );
        let mut outputs = vec![
            output("bbbb").as_str().to_string(),
            output("cccc").as_str().to_string(),
        ];
        outputs.sort();
        assert_eq!(left(&root, OUTPUTS_DIR), outputs);
        let mut keys = vec![
            key("bbbb").as_str().to_string(),
            key("cccc").as_str().to_string(),
        ];
        keys.sort();
        assert_eq!(left(&root, CACHE_DIR), keys);
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
        kept(&root, "aaaa");
        sweep(&root, 0);
        assert!(going.exists(), "a run whose process is alive is kept");
        assert!(
            !stale.exists(),
            "a marker from a process that is gone keeps nothing"
        );
        assert!(root
            .join(OUTPUTS_DIR)
            .join(output("aaaa").as_str())
            .exists());
        assert!(root.join(CACHE_DIR).join(key("aaaa").as_str()).exists());
        let _ = fs::remove_dir_all(&root);
    }
}
