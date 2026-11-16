pub mod committed;
mod gitignore;

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::path::PathBuf;

use crate::core::digest::Digest;
use crate::core::listing::gitignore::Gitignore;
use crate::domain::file::File;

// The files under one folder that no rule keeps out, sorted by path, the
// folders below it holding a repository of their own, and how many entries
// the walk passed by and why.
pub struct Listing {
    pub files: Vec<File>,
    pub foreign: Vec<PathBuf>,
    pub bytes: u64,
    pub ignored: u64,
    pub skipped: u64,
    pub links: u64,
}

// Every file under the folder that no rule keeps out. Paths run from the
// folder, and a digest stands for a file's content, so two listings differ
// exactly when the files do. What a .gitignore keeps out is out unless git
// carries it; a folder of an ecosystem's build output is out whatever git
// carries in it. The checkpoint is asked before each folder is read.
pub fn list(
    root: &Path,
    outputs: &[&str],
    committed: &BTreeSet<PathBuf>,
    checkpoint: &mut dyn FnMut() -> Result<(), String>,
) -> Result<Listing, String> {
    if !root.is_absolute() {
        return Err(format!(
            "the folder {} is not an absolute path",
            root.display()
        ));
    }
    if !root.is_dir() {
        return Err(format!("no folder at {}", root.display()));
    }
    let mut walk = Walk {
        root,
        outputs,
        committed,
        rules: Vec::new(),
        buffer: vec![0; 64 * 1024],
        files: Vec::new(),
        bytes: 0,
        ignored: 0,
        skipped: 0,
        foreign: Vec::new(),
        links: 0,
    };
    walk.folder(checkpoint, root, 0, false)?;
    let Walk {
        mut files,
        bytes,
        ignored,
        skipped,
        mut foreign,
        links,
        ..
    } = walk;
    files.sort_by(|one, other| one.path.cmp(&other.path));
    foreign.sort();
    Ok(Listing {
        files,
        foreign,
        bytes,
        ignored,
        skipped,
        links,
    })
}

struct Walk<'root> {
    root: &'root Path,
    outputs: &'root [&'root str],
    committed: &'root BTreeSet<PathBuf>,
    // Each `.gitignore` met on the way down to the folder being read, with
    // how many names deep its own folder is.
    rules: Vec<(usize, Gitignore)>,
    buffer: Vec<u8>,
    files: Vec<File>,
    bytes: u64,
    ignored: u64,
    skipped: u64,
    foreign: Vec<PathBuf>,
    links: u64,
}

impl Walk<'_> {
    // A link is never followed, so nothing outside the folder is read through
    // one and nothing inside it twice. A folder named `third_party` or by a
    // leading dot is never read, nor an ecosystem's build output, nor one
    // holding a `.arbiter` of its own, whatever a rule says or git carries. Anything that is not a plain file is never
    // opened: reading a pipe would wait for ever.
    fn folder(
        &mut self,
        checkpoint: &mut dyn FnMut() -> Result<(), String>,
        at: &Path,
        depth: usize,
        shut: bool,
    ) -> Result<(), String> {
        checkpoint()?;
        let ruled = self.ruled(at, depth)?;
        let entries = fs::read_dir(at).map_err(|error| unread(at, &error))?;
        for entry in entries {
            let entry = entry.map_err(|error| unread(at, &error))?;
            let path = entry.path();
            let kind = entry.file_type().map_err(|error| unread(&path, &error))?;
            if kind.is_symlink() {
                self.links += 1;
                continue;
            }
            let name = entry.file_name();
            if kind.is_dir()
                && (name == "third_party"
                    || name.as_bytes().starts_with(b".")
                    || name
                        .to_str()
                        .is_some_and(|name| self.outputs.contains(&name)))
            {
                self.skipped += 1;
                continue;
            }
            let relative = path
                .strip_prefix(self.root)
                .map_err(|_| format!("{} is not under the folder", path.display()))?
                .to_path_buf();
            if kind.is_dir() && own_repository(&path)? {
                self.foreign.push(relative);
                continue;
            }
            // Inside a folder a rule keeps out, only what git carries is in.
            let kept_out = shut || self.ignores(&relative, kind.is_dir());
            if kept_out && !self.carried(&relative, kind.is_dir()) {
                self.ignored += 1;
                continue;
            }
            if kind.is_dir() {
                self.folder(checkpoint, &path, depth + 1, kept_out)?;
            } else if kind.is_file() {
                self.file(&path, relative)?;
            }
        }
        if ruled {
            self.rules.pop();
        }
        Ok(())
    }

    // The folder's own `.gitignore`, when it holds one as a plain file, joins
    // the rules for as long as the folder is being read.
    fn ruled(&mut self, at: &Path, depth: usize) -> Result<bool, String> {
        let own = at.join(".gitignore");
        match fs::symlink_metadata(&own) {
            Ok(found) if found.is_file() => {
                let text = fs::read(&own).map_err(|error| unread(&own, &error))?;
                self.rules.push((depth, Gitignore::parsed(&text)));
                Ok(true)
            }
            Ok(_) => Ok(false),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(unread(&own, &error)),
        }
    }

    // The deepest `.gitignore` with a rule matching the path decides, and one
    // above it is asked only when it has none. Each is asked by the path from
    // its own folder.
    fn ignores(&self, relative: &Path, folder: bool) -> bool {
        let names: Vec<&[u8]> = relative
            .components()
            .map(|part| part.as_os_str().as_bytes())
            .collect();
        self.rules
            .iter()
            .rev()
            .find_map(|(depth, rules)| rules.ignores(&names[*depth..], folder))
            .unwrap_or(false)
    }

    // Whether git carries the path, or for a folder anything under it: a
    // committed file is in the repository whatever a pattern says.
    fn carried(&self, relative: &Path, folder: bool) -> bool {
        if folder {
            self.committed
                .range(relative.to_path_buf()..)
                .next()
                .is_some_and(|carried| carried.starts_with(relative))
        } else {
            self.committed.contains(relative)
        }
    }

    // Read through one buffer, so a file larger than memory is still read.
    fn file(&mut self, path: &Path, relative: PathBuf) -> Result<(), String> {
        let mut source = fs::File::open(path).map_err(|error| unread(path, &error))?;
        let mut digest = Digest::new();
        let mut bytes = 0u64;
        loop {
            match source.read(&mut self.buffer) {
                Ok(0) => break,
                Ok(read) => {
                    digest.eat(&self.buffer[..read]);
                    bytes += read as u64;
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(unread(path, &error)),
            }
        }
        self.bytes += bytes;
        self.files.push(File {
            path: relative,
            bytes,
            digest: digest.hex(),
        });
        Ok(())
    }
}

// A folder holding a `.arbiter` folder of its own is a repository of its
// own, gated from its own root: this run names it and reads nothing under
// it. A link of that name counts for nothing, as any link does.
fn own_repository(folder: &Path) -> Result<bool, String> {
    let own = folder.join(".arbiter");
    match fs::symlink_metadata(&own) {
        Ok(found) => Ok(found.is_dir()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(unread(&own, &error)),
    }
}

fn unread(path: &Path, error: &io::Error) -> String {
    format!("{} could not be read: {error}", path.display())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::env;
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use std::process::Command;

    use super::committed::committed;
    use super::list;

    fn folder(name: &str, written: &[(&str, &str)]) -> PathBuf {
        let root = env::temp_dir().join(format!("arbiter-listing-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for (path, content) in written {
            let file = root.join(path);
            fs::create_dir_all(file.parent().expect("a file sits in a folder"))
                .expect("the folder is made");
            fs::write(file, content).expect("the file is written");
        }
        fs::canonicalize(&root).expect("the folder resolves")
    }

    fn listed(root: &Path, outputs: &[&str], committed: &BTreeSet<PathBuf>) -> Vec<String> {
        list(root, outputs, committed, &mut || Ok(()))
            .expect("the folder is read")
            .files
            .iter()
            .map(|file| file.path.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn a_gitignore_keeps_out_what_it_names_with_or_without_git() {
        let root = folder(
            "ruled",
            &[
                (".gitignore", "*.log\n"),
                ("kept.rs", ""),
                ("noise.log", ""),
            ],
        );
        assert_eq!(
            listed(&root, &[], &BTreeSet::new()),
            [".gitignore", "kept.rs"]
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_build_output_folder_is_out_whatever_a_rule_says() {
        let root = folder("output", &[("src/lib.rs", ""), ("target/debug/built", "")]);
        let listing = list(&root, &["target"], &BTreeSet::new(), &mut || Ok(())).expect("read");
        let paths: Vec<String> = listing
            .files
            .iter()
            .map(|file| file.path.to_string_lossy().into_owned())
            .collect();
        assert_eq!(paths, ["src/lib.rs"]);
        assert_eq!(listing.skipped, 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_file_git_carries_is_listed_whatever_a_pattern_says() {
        let root = folder(
            "carried",
            &[
                (".gitignore", "*.log\nbuilt/\n"),
                ("kept.log", "committed"),
                ("built/shipped.txt", "committed"),
                ("built/stray.txt", "not committed"),
                ("noise.log", "not committed"),
            ],
        );
        let git = |args: &[&str]| {
            Command::new("git")
                .arg("-C")
                .arg(&root)
                .args(args)
                .output()
                .expect("git runs")
        };
        assert!(
            git(&["init", "-q"]).status.success(),
            "a repository is made"
        );
        assert!(
            git(&["add", "-f", ".gitignore", "kept.log", "built/shipped.txt"])
                .status
                .success()
        );
        let carried = committed(&root);
        assert_eq!(
            listed(&root, &[], &carried),
            [".gitignore", "built/shipped.txt", "kept.log"],
            "git carries what the index holds, and nothing else it was shown"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_folder_git_does_not_hold_carries_nothing() {
        let root = folder("bare", &[("a.rs", "")]);
        assert!(committed(&root).is_empty());
        let _ = fs::remove_dir_all(&root);
    }
}
