use task_runner::context::task_context::TaskContext;
use task_runner::record::task_failure::TaskFailure;

use crate::core::listing::committed::committed;
use crate::core::listing::list;
use crate::core::listing::Listing;
use crate::domain::folder::Folder;

// The listing of the folder, each folder holding a repository of its own
// noted and the walk's counts measured.
pub fn list_files(context: &mut TaskContext, folder: &Folder) -> Result<Listing, TaskFailure> {
    let languages = crate::packs::all();
    let outputs: Vec<&str> = languages
        .iter()
        .flat_map(|language| language.outputs().iter().copied())
        .collect();
    let carried = committed(&folder.path);
    let listing = list(&folder.path, &outputs, &carried, &mut || {
        context.checkpoint().map_err(|failure| failure.reason)
    })
    .map_err(TaskFailure::from)?;
    for foreign in &listing.foreign {
        context.note(format!("foreign: {}", foreign.display()));
    }
    context.measure("files", listing.files.len() as f64);
    context.measure("bytes", listing.bytes as f64);
    context.measure("ignored", listing.ignored as f64);
    context.measure("skipped", listing.skipped as f64);
    context.measure("foreign", listing.foreign.len() as f64);
    context.measure("links", listing.links as f64);
    Ok(listing)
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

    use task_runner::context::stop_token::StopToken;
    use task_runner::context::task_context::TaskContext;
    use task_runner::contract::RunId;
    use task_runner::contract::RunRecord;
    use task_runner::contract::TaskAttemptId;

    use super::list_files;
    use crate::core::digest::Digest;
    use crate::domain::file::File;
    use crate::domain::folder::Folder;

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

    fn context() -> TaskContext {
        TaskContext::new(
            TaskAttemptId::new(RunId::next(), "list_files", 1),
            StopToken::new(),
            env::temp_dir(),
        )
    }

    // The folder listed, a failure told by its reason.
    fn performed(context: &mut TaskContext, root: &Path) -> Result<Vec<File>, String> {
        let folder = Folder {
            path: root.to_path_buf(),
        };
        list_files(context, &folder)
            .map(|listing| listing.files)
            .map_err(|failure| failure.reason)
    }

    fn paths(listed: &[File]) -> Vec<String> {
        listed
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
        let mut context = context();
        let listed = performed(&mut context, &root).expect("the folder is read");
        assert_eq!(paths(&listed), ["b.rs", "src/a.rs"]);
        assert_eq!(listed[0].bytes, 11);
        assert_eq!(listed[0].digest, digest("fn b() {}\n\n"));
        assert_eq!(listed[1].bytes, 10);
        assert_eq!(listed[1].digest, digest("fn a() {}\n"));
        let attempt = context.finished(RunRecord::passed());
        assert_eq!(attempt.measures.get("files"), Some(&2.0));
        assert_eq!(attempt.measures.get("bytes"), Some(&21.0));
        assert_eq!(attempt.measures.get("ignored"), Some(&0.0));
        assert_eq!(attempt.measures.get("skipped"), Some(&0.0));
        assert_eq!(attempt.measures.get("foreign"), Some(&0.0));
        assert_eq!(attempt.measures.get("links"), Some(&0.0));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_listing_changes_exactly_when_a_file_s_content_does() {
        let root = folder("changed", &[("a.rs", "one\n")]);
        let first = performed(&mut context(), &root).expect("the folder is read");
        fs::write(root.join("a.rs"), "one\n").expect("the file is rewritten the same");
        let same = performed(&mut context(), &root).expect("the folder is read");
        assert_eq!(same, first, "the same content lists the same");
        fs::write(root.join("a.rs"), "two\n").expect("the file is rewritten");
        let other = performed(&mut context(), &root).expect("the folder is read");
        assert_eq!(other[0].bytes, first[0].bytes);
        assert_eq!(other[0].digest, digest("two\n"));
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
        let mut context = context();
        let listed = performed(&mut context, &root).expect("the folder is read");
        assert_eq!(paths(&listed), ["real/a.rs"]);
        let attempt = context.finished(RunRecord::passed());
        assert_eq!(attempt.measures.get("links"), Some(&2.0));
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
        let mut context = context();
        let listed = performed(&mut context, &root).expect("the folder is read");
        assert_eq!(
            paths(&listed),
            [".env", "docs/third_party", "src/a.rs"],
            "the names are folders': a file named so stays"
        );
        let attempt = context.finished(RunRecord::passed());
        assert_eq!(attempt.measures.get("skipped"), Some(&4.0));
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
        let mut context = context();
        let listed = performed(&mut context, &root).expect("the folder is read");
        assert_eq!(
            paths(&listed),
            ["src/a.rs", "tests/fixtures/plain/y.rs"],
            "the folder pointed at is read whatever it holds"
        );
        let attempt = context.finished(RunRecord::passed());
        assert_eq!(
            attempt.measures.get("foreign"),
            Some(&2.0),
            "one inside another is never reached"
        );
        assert_eq!(attempt.measures.get("skipped"), Some(&1.0));
        assert_eq!(
            attempt.evidence,
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
        let mut context = context();
        let listed = performed(&mut context, &root).expect("the folder is read");
        assert_eq!(paths(&listed), [".gitignore", "a.rs"]);
        let attempt = context.finished(RunRecord::passed());
        assert_eq!(attempt.measures.get("foreign"), Some(&1.0));
        assert_eq!(attempt.measures.get("ignored"), Some(&0.0));
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
        let mut context = context();
        let listed = performed(&mut context, &root).expect("the folder is read");
        assert_eq!(
            paths(&listed),
            ["one/.arbiter", "one/x.rs", "records/units.toml", "two/y.rs"]
        );
        let attempt = context.finished(RunRecord::passed());
        assert_eq!(attempt.measures.get("foreign"), Some(&0.0));
        assert_eq!(attempt.measures.get("links"), Some(&1.0));
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
        let mut context = context();
        let listed = performed(&mut context, &root).expect("the folder is read");
        assert_eq!(paths(&listed), [".gitignore", "kept.rs", "sub/kept.txt"]);
        let attempt = context.finished(RunRecord::passed());
        assert_eq!(
            attempt.measures.get("ignored"),
            Some(&3.0),
            "a folder left out counts once"
        );
        assert_eq!(attempt.measures.get("files"), Some(&3.0));
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
        let listed = performed(&mut context(), &root).expect("the folder is read");
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
        let listed = performed(&mut context(), &root).expect("the folder is read");
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
        let listed = performed(&mut context(), &root).expect("the folder is read");
        assert_eq!(paths(&listed), ["a.rs", "rules"]);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn an_entry_that_is_neither_a_file_nor_a_folder_is_not_listed() {
        let root = folder("kinds", &[("a.rs", "a\n")]);
        let _socket = UnixListener::bind(root.join("s")).expect("a socket is bound");
        let listed = performed(&mut context(), &root).expect("the folder is read");
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
            let failed = performed(&mut context(), &root).expect_err("the listing is not whole");
            assert!(failed.contains("locked.rs could not be read"), "{failed}");
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_folder_that_is_not_absolute_or_not_there_is_refused_before_anything_is_read() {
        let root = folder("refused", &[("a.rs", "a\n")]);
        let missing = performed(&mut context(), &root.join("missing"))
            .expect_err("a folder that is not there is refused");
        assert!(missing.starts_with("no folder at "), "{missing}");
        let file =
            performed(&mut context(), &root.join("a.rs")).expect_err("a file is not a folder");
        assert!(file.starts_with("no folder at "), "{file}");
        let relative = performed(&mut context(), Path::new("relative"))
            .expect_err("a path that is not absolute is refused");
        assert!(relative.contains("is not an absolute path"), "{relative}");
        let _ = fs::remove_dir_all(&root);
    }
}
