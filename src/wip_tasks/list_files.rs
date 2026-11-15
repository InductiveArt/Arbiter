use std::fs;
use std::io;
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

use crate::core::digest::Digest;
use crate::wip::pull::In;
use crate::wip::scope::Scope;
use crate::wip::task::Task;
use crate::wip::trace::outcome::Failure;
use crate::wip_tasks::gitignore::Gitignore;

// The folder a run is pointed at, by its absolute path: the process reading
// it runs elsewhere than whoever wrote the path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Folder {
    pub path: PathBuf,
}

// Every file under the folder that no rule keeps out, sorted by path. Paths
// run from the folder, and a digest stands for a file's content, so two
// listings differ exactly when the files do.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Files {
    pub files: Vec<File>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct File {
    pub path: PathBuf,
    pub bytes: u64,
    pub digest: String,
}

pub fn list_files_task() -> Task {
    Task::new("list_files", |scope: &mut Scope, folder: In<Folder>| {
        list_files(scope, &folder)
    })
    .line(
        "every file under one folder with its size and the digest of its content, less what a \
         .gitignore leaves out; third_party, dot-folders, links and any folder below it holding \
         its own .arbiter are never read",
    )
}

fn list_files(scope: &mut Scope, folder: &Folder) -> Result<Files, Failure> {
    if !folder.path.is_absolute() {
        return Err(Failure::from(format!(
            "the folder {} is not an absolute path",
            folder.path.display()
        )));
    }
    if !folder.path.is_dir() {
        return Err(Failure::from(format!(
            "no folder at {}",
            folder.path.display()
        )));
    }
    let mut reading = Reading {
        root: &folder.path,
        rules: Vec::new(),
        buffer: vec![0; 64 * 1024],
        files: Vec::new(),
        bytes: 0,
        ignored: 0,
        skipped: 0,
        foreign: Vec::new(),
        links: 0,
    };
    reading.folder(scope, &folder.path, 0)?;
    let Reading {
        mut files,
        bytes,
        ignored,
        skipped,
        mut foreign,
        links,
        ..
    } = reading;
    files.sort_by(|one, other| one.path.cmp(&other.path));
    foreign.sort();
    for folder in &foreign {
        scope.note(format!("foreign: {}", folder.display()));
    }
    scope.measure("files", files.len() as f64);
    scope.measure("bytes", bytes as f64);
    scope.measure("ignored", ignored as f64);
    scope.measure("skipped", skipped as f64);
    scope.measure("foreign", foreign.len() as f64);
    scope.measure("links", links as f64);
    Ok(Files { files })
}

struct Reading<'root> {
    root: &'root Path,
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

impl Reading<'_> {
    // A link is never followed, so nothing outside the folder is read through
    // one and nothing inside it twice. A folder named `third_party` or by a
    // leading dot is never read, nor one holding a `.arbiter` of its own,
    // whatever a rule says. Anything that is not a plain file is never
    // opened: reading a pipe would wait for ever.
    fn folder(&mut self, scope: &mut Scope, at: &Path, depth: usize) -> Result<(), Failure> {
        scope.checkpoint()?;
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
            if kind.is_dir() && (name == "third_party" || name.as_bytes().starts_with(b".")) {
                self.skipped += 1;
                continue;
            }
            let relative = path
                .strip_prefix(self.root)
                .map_err(|_| Failure::from(format!("{} is not under the folder", path.display())))?
                .to_path_buf();
            if kind.is_dir() && judged_apart(&path)? {
                self.foreign.push(relative);
                continue;
            }
            if self.ignores(&relative, kind.is_dir()) {
                self.ignored += 1;
                continue;
            }
            if kind.is_dir() {
                self.folder(scope, &path, depth + 1)?;
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
    fn ruled(&mut self, at: &Path, depth: usize) -> Result<bool, Failure> {
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

    // Read through one buffer, so a file larger than memory is still read.
    fn file(&mut self, path: &Path, relative: PathBuf) -> Result<(), Failure> {
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

// A folder holding a `.arbiter` folder of its own is judged from its own
// root: this run names it and reads nothing under it. A link of that name
// counts for nothing, as any link does.
fn judged_apart(folder: &Path) -> Result<bool, Failure> {
    let own = folder.join(".arbiter");
    match fs::symlink_metadata(&own) {
        Ok(found) => Ok(found.is_dir()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(unread(&own, &error)),
    }
}

fn unread(path: &Path, error: &io::Error) -> Failure {
    Failure::from(format!("{} could not be read: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;
    use std::path::Path;
    use std::path::PathBuf;

    use super::list_files_task;
    use super::Files;
    use super::Folder;
    use crate::core::digest::Digest;
    use crate::wip::job::cancel::Cancel;
    use crate::wip::job::id::JobId;
    use crate::wip::job::id::RunId;
    use crate::wip::layout::Site;
    use crate::wip::scope::Scope;
    use crate::wip::store::Part;
    use crate::wip::store::Product;
    use crate::wip::trace::outcome::Outcome;

    // A folder of its own per test, holding the files written.
    fn folder(name: &str, written: &[(&str, &str)]) -> PathBuf {
        let root = env::temp_dir().join(format!(
            "arbiter-list-{name}-{}-{}",
            std::process::id(),
            RunId::next()
        ));
        fs::create_dir_all(&root).expect("the folder is made");
        for (path, content) in written {
            let file = root.join(path);
            fs::create_dir_all(file.parent().expect("a file sits in a folder"))
                .expect("the folder is made");
            fs::write(file, content).expect("the file is written");
        }
        root
    }

    fn scope() -> Scope {
        let site = Site {
            dir: env::temp_dir(),
            store: env::temp_dir(),
        };
        Scope::new(
            JobId::new(RunId::next(), "list_files", 1),
            Cancel::new(),
            site,
        )
    }

    // The task performed as a worker performs it: the folder handed encoded,
    // the product decoded, a failure told by its reason.
    fn performed(scope: &mut Scope, root: &Path) -> Result<Files, String> {
        let folder = Folder {
            path: root.to_path_buf(),
        };
        let part = Part {
            value: Some(Product::encoded(&folder).expect("a folder encodes")),
            completion: None,
        };
        list_files_task()
            .perform(scope, vec![part])
            .map(|bytes| serde_json::from_slice(&bytes).expect("the product decodes"))
            .map_err(|failure| failure.reason)
    }

    fn paths(listed: &Files) -> Vec<String> {
        listed
            .files
            .iter()
            .map(|file| file.path.to_string_lossy().into_owned())
            .collect()
    }

    fn digest(content: &str) -> String {
        let mut digest = Digest::new();
        digest.eat(content.as_bytes());
        digest.hex()
    }

    #[test]
    fn every_file_under_the_folder_is_listed_by_path_with_its_size_and_digest() {
        let root = folder(
            "listed",
            &[("src/a.rs", "fn a() {}\n"), ("b.rs", "fn b() {}\n\n")],
        );
        let mut scope = scope();
        let listed = performed(&mut scope, &root).expect("the folder is read");
        assert_eq!(paths(&listed), ["b.rs", "src/a.rs"]);
        assert_eq!(listed.files[0].bytes, 11);
        assert_eq!(listed.files[0].digest, digest("fn b() {}\n\n"));
        assert_eq!(listed.files[1].bytes, 10);
        assert_eq!(listed.files[1].digest, digest("fn a() {}\n"));
        let step = scope.finished("list_files", Outcome::Ok);
        assert_eq!(step.measures.get("files"), Some(&2.0));
        assert_eq!(step.measures.get("bytes"), Some(&21.0));
        assert_eq!(step.measures.get("ignored"), Some(&0.0));
        assert_eq!(step.measures.get("skipped"), Some(&0.0));
        assert_eq!(step.measures.get("foreign"), Some(&0.0));
        assert_eq!(step.measures.get("links"), Some(&0.0));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_listing_changes_exactly_when_a_file_s_content_does() {
        let root = folder("changed", &[("a.rs", "one\n")]);
        let first = performed(&mut scope(), &root).expect("the folder is read");
        fs::write(root.join("a.rs"), "one\n").expect("the file is rewritten the same");
        let same = performed(&mut scope(), &root).expect("the folder is read");
        assert_eq!(same, first, "the same content lists the same");
        fs::write(root.join("a.rs"), "two\n").expect("the file is rewritten");
        let other = performed(&mut scope(), &root).expect("the folder is read");
        assert_eq!(other.files[0].bytes, first.files[0].bytes);
        assert_eq!(other.files[0].digest, digest("two\n"));
        assert_ne!(
            other, first,
            "another content of the same size lists differently"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_link_is_neither_followed_nor_listed() {
        let root = folder("linked", &[("real/a.rs", "a\n")]);
        symlink(root.join("real"), root.join("alias")).expect("the folder is linked");
        symlink(root.join("real/a.rs"), root.join("a-link.rs")).expect("the file is linked");
        let mut scope = scope();
        let listed = performed(&mut scope, &root).expect("the folder is read");
        assert_eq!(paths(&listed), ["real/a.rs"]);
        let step = scope.finished("list_files", Outcome::Ok);
        assert_eq!(step.measures.get("links"), Some(&2.0));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_dot_folder_and_third_party_are_never_read_wherever_they_sit() {
        let root = folder(
            "skipped",
            &[
                ("src/a.rs", "a\n"),
                (".env", "KEY=1\n"),
                (".tooling/state", "x"),
                ("deep/.cache/x", "x"),
                ("third_party/lib/x.rs", "x"),
                ("deep/third_party/y.rs", "y"),
                ("docs/third_party", "a file of that name\n"),
            ],
        );
        let mut scope = scope();
        let listed = performed(&mut scope, &root).expect("the folder is read");
        assert_eq!(
            paths(&listed),
            [".env", "docs/third_party", "src/a.rs"],
            "the names are folders': a file named so stays"
        );
        let step = scope.finished("list_files", Outcome::Ok);
        assert_eq!(step.measures.get("skipped"), Some(&4.0));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_folder_below_holding_its_own_arbiter_is_named_and_never_read() {
        let root = folder(
            "foreign",
            &[
                (".arbiter/units.toml", ""),
                ("src/a.rs", "a\n"),
                ("exhibits/cinema/.arbiter/units.toml", ""),
                ("exhibits/cinema/app.py", "x = 1\n"),
                ("exhibits/cinema/inner/.arbiter/units.toml", ""),
                ("exhibits/cinema/inner/b.py", "y = 1\n"),
                ("tests/fixtures/demo/.arbiter/units.toml", ""),
                ("tests/fixtures/demo/x.rs", "x\n"),
                ("tests/fixtures/plain/y.rs", "y\n"),
            ],
        );
        let mut scope = scope();
        let listed = performed(&mut scope, &root).expect("the folder is read");
        assert_eq!(
            paths(&listed),
            ["src/a.rs", "tests/fixtures/plain/y.rs"],
            "the folder pointed at is read whatever it holds"
        );
        let step = scope.finished("list_files", Outcome::Ok);
        assert_eq!(
            step.measures.get("foreign"),
            Some(&2.0),
            "one inside another is never reached"
        );
        assert_eq!(step.measures.get("skipped"), Some(&1.0));
        assert_eq!(
            step.evidence,
            ["foreign: exhibits/cinema", "foreign: tests/fixtures/demo"]
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_gitignore_does_not_speak_for_a_folder_holding_its_own_arbiter() {
        let root = folder(
            "foreign-ignored",
            &[
                (".gitignore", "/other\n"),
                ("other/.arbiter/units.toml", ""),
                ("other/x.rs", "x\n"),
                ("a.rs", "a\n"),
            ],
        );
        let mut scope = scope();
        let listed = performed(&mut scope, &root).expect("the folder is read");
        assert_eq!(paths(&listed), [".gitignore", "a.rs"]);
        let step = scope.finished("list_files", Outcome::Ok);
        assert_eq!(step.measures.get("foreign"), Some(&1.0));
        assert_eq!(step.measures.get("ignored"), Some(&0.0));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_file_or_a_link_named_arbiter_is_no_barrier() {
        let root = folder(
            "no-barrier",
            &[
                ("one/.arbiter", "a file of that name\n"),
                ("one/x.rs", "x\n"),
                ("two/y.rs", "y\n"),
                ("records/units.toml", ""),
            ],
        );
        symlink(root.join("records"), root.join("two/.arbiter")).expect("the folder is linked");
        let mut scope = scope();
        let listed = performed(&mut scope, &root).expect("the folder is read");
        assert_eq!(
            paths(&listed),
            ["one/.arbiter", "one/x.rs", "records/units.toml", "two/y.rs"]
        );
        let step = scope.finished("list_files", Outcome::Ok);
        assert_eq!(step.measures.get("foreign"), Some(&0.0));
        assert_eq!(step.measures.get("links"), Some(&1.0));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn what_a_gitignore_names_is_left_out_with_no_repository_around() {
        let root = folder(
            "ignored",
            &[
                (".gitignore", "*.log\nout/\n"),
                ("kept.rs", "k\n"),
                ("noise.log", "n\n"),
                ("out/built.bin", "b"),
                ("sub/deep.log", "d\n"),
                ("sub/kept.txt", "t\n"),
            ],
        );
        let mut scope = scope();
        let listed = performed(&mut scope, &root).expect("the folder is read");
        assert_eq!(paths(&listed), [".gitignore", "kept.rs", "sub/kept.txt"]);
        let step = scope.finished("list_files", Outcome::Ok);
        assert_eq!(
            step.measures.get("ignored"),
            Some(&3.0),
            "a folder left out counts once"
        );
        assert_eq!(step.measures.get("files"), Some(&3.0));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_deeper_gitignore_speaks_for_its_own_folder_and_wins_over_one_above() {
        let root = folder(
            "deeper",
            &[
                (".gitignore", "*.log\n"),
                ("a.log", "a\n"),
                ("sub/.gitignore", "!keep.log\n*.tmp\n"),
                ("sub/keep.log", "k\n"),
                ("sub/drop.log", "d\n"),
                ("sub/x.tmp", "x\n"),
                ("sub/inner/keep.log", "k\n"),
                ("other/x.tmp", "x\n"),
                ("other/keep.log", "k\n"),
            ],
        );
        let listed = performed(&mut scope(), &root).expect("the folder is read");
        assert_eq!(
            paths(&listed),
            [
                ".gitignore",
                "other/x.tmp",
                "sub/.gitignore",
                "sub/inner/keep.log",
                "sub/keep.log"
            ]
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_pattern_tied_by_a_slash_is_taken_from_its_own_file_s_folder() {
        let root = folder(
            "tied",
            &[
                ("sub/.gitignore", "/build\n"),
                ("sub/build/x", "x"),
                ("sub/deep/build/y", "y"),
                ("build/z", "z"),
            ],
        );
        let listed = performed(&mut scope(), &root).expect("the folder is read");
        assert_eq!(
            paths(&listed),
            ["build/z", "sub/.gitignore", "sub/deep/build/y"]
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_gitignore_that_is_a_link_gives_no_rule() {
        let root = folder("linked-rules", &[("rules", "*.rs\n"), ("a.rs", "a\n")]);
        symlink(root.join("rules"), root.join(".gitignore")).expect("the file is linked");
        let listed = performed(&mut scope(), &root).expect("the folder is read");
        assert_eq!(paths(&listed), ["a.rs", "rules"]);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn an_entry_that_is_neither_a_file_nor_a_folder_is_not_listed() {
        let root = folder("kinds", &[("a.rs", "a\n")]);
        let _socket = UnixListener::bind(root.join("s")).expect("a socket is bound");
        let listed = performed(&mut scope(), &root).expect("the folder is read");
        assert_eq!(paths(&listed), ["a.rs"]);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_file_that_cannot_be_read_fails_the_attempt() {
        let root = folder("locked", &[("a.rs", "a\n"), ("locked.rs", "l\n")]);
        let locked = root.join("locked.rs");
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000))
            .expect("the file is locked");
        // An account no permission stops reads the file anyway: nothing to show.
        if fs::File::open(&locked).is_err() {
            let failed = performed(&mut scope(), &root).expect_err("the listing is not whole");
            assert!(failed.contains("locked.rs could not be read"), "{failed}");
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_folder_that_is_not_absolute_or_not_there_is_refused_before_anything_is_read() {
        let root = folder("refused", &[("a.rs", "a\n")]);
        let missing = performed(&mut scope(), &root.join("missing"))
            .expect_err("a folder that is not there is refused");
        assert!(missing.starts_with("no folder at "), "{missing}");
        let file = performed(&mut scope(), &root.join("a.rs")).expect_err("a file is not a folder");
        assert!(file.starts_with("no folder at "), "{file}");
        let relative = performed(&mut scope(), Path::new("relative"))
            .expect_err("a path that is not absolute is refused");
        assert!(relative.contains("is not an absolute path"), "{relative}");
        let _ = fs::remove_dir_all(&root);
    }
}
