use task_runner::context::task_context::TaskContext;
use task_runner::record::task_failure::TaskFailure;

use crate::core::markers;
use crate::domain::ecosystem::Ecosystem;
use crate::domain::file::File;
use crate::domain::folder::Folder;
use crate::domain::marker::Marker;

// Every marker among the listed files, as the markers are read, with how many
// there are and how many only list members.
pub fn read_markers(
    context: &mut TaskContext,
    folder: &Folder,
    files: &[File],
    ecosystems: &[Ecosystem],
) -> Result<Vec<Marker>, TaskFailure> {
    let found = markers::read_markers(&folder.path, files, ecosystems, &mut || {
        context.checkpoint().map_err(|failure| failure.reason)
    })
    .map_err(TaskFailure::from)?;
    let only_list = found.iter().filter(|marker| marker.members_only).count();
    context.measure("markers", found.len() as f64);
    context.measure("members_only", only_list as f64);
    Ok(found)
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;

    use task_runner::context::stop_token::StopToken;
    use task_runner::context::task_context::TaskContext;
    use task_runner::contract::RunId;
    use task_runner::contract::RunRecord;
    use task_runner::contract::TaskAttemptId;

    use super::read_markers;
    use crate::core::digest::Digest;
    use crate::domain::ecosystem::Ecosystem;
    use crate::domain::ecosystem::MembersOnly;
    use crate::domain::file::File;
    use crate::domain::folder::Folder;
    use crate::domain::marker::Marker;

    // A folder of its own per test holding the files written, with the list
    // a listing of it leaves.
    fn written(name: &str, files: &[(&str, &str)]) -> (PathBuf, Vec<File>) {
        let root = env::temp_dir().join(format!(
            "arbiter-markers-{name}-{}-{}",
            std::process::id(),
            RunId::next()
        ));
        fs::create_dir_all(&root).expect("the folder is made");
        let mut listed = Vec::new();
        for (path, content) in files {
            let file = root.join(path);
            fs::create_dir_all(file.parent().expect("a file sits in a folder"))
                .expect("the folder is made");
            fs::write(&file, content).expect("the file is written");
            let mut digest = Digest::new();
            digest.eat(content.as_bytes());
            listed.push(File {
                path: PathBuf::from(path),
                bytes: content.len() as u64,
                digest: digest.hex(),
            });
        }
        (root, listed)
    }

    fn ecosystem(name: &str, marker: &str, rule: Option<(&[&str], &[&str])>) -> Ecosystem {
        Ecosystem {
            name: name.to_string(),
            marker: marker.to_string(),
            members_only: rule.map(|(has, lacks)| MembersOnly {
                has: has.iter().map(|key| key.to_string()).collect(),
                lacks: lacks.iter().map(|key| key.to_string()).collect(),
            }),
            tools: Vec::new(),
            configs: Vec::new(),
        }
    }

    fn ecosystems() -> Vec<Ecosystem> {
        vec![
            ecosystem("proto", "buf.yaml", None),
            ecosystem(
                "python",
                "pyproject.toml",
                Some((&["tool.uv.workspace"], &["project"])),
            ),
            ecosystem("rust", "Cargo.toml", Some((&["workspace"], &["package"]))),
            ecosystem(
                "typescript",
                "package.json",
                Some((&["workspaces"], &["main", "exports", "bin"])),
            ),
        ]
    }

    fn context() -> TaskContext {
        TaskContext::new(
            TaskAttemptId::new(RunId::next(), "read_markers", 1),
            StopToken::new(),
            env::temp_dir(),
        )
    }

    // The markers read, a failure told by its reason.
    fn performed(
        context: &mut TaskContext,
        root: &Path,
        files: &[File],
        ecosystems: &[Ecosystem],
    ) -> Result<Vec<Marker>, String> {
        let folder = Folder {
            path: root.to_path_buf(),
        };
        read_markers(context, &folder, files, ecosystems).map_err(|failure| failure.reason)
    }

    fn marker(path: &str, ecosystem: &str, members_only: bool) -> Marker {
        Marker {
            path: PathBuf::from(path),
            ecosystem: ecosystem.to_string(),
            members_only,
        }
    }

    #[test]
    fn a_listed_file_named_as_a_marker_is_told_with_its_ecosystem() {
        let (root, listed) = written(
            "told",
            &[
                ("Cargo.toml", "[package]\nname = \"a\"\n"),
                ("src/lib.rs", "pub fn a() {}\n"),
                ("proto/buf.yaml", "version: v2\n"),
                (
                    "tools/probe/pyproject.toml",
                    "[project]\nname = \"probe\"\n",
                ),
                (
                    "web/package.json",
                    "{\"name\": \"web\", \"main\": \"index.js\"}\n",
                ),
            ],
        );
        let mut context = context();
        let markers =
            performed(&mut context, &root, &listed, &ecosystems()).expect("the markers are read");
        assert_eq!(
            markers,
            [
                marker("Cargo.toml", "rust", false),
                marker("proto/buf.yaml", "proto", false),
                marker("tools/probe/pyproject.toml", "python", false),
                marker("web/package.json", "typescript", false),
            ]
        );
        let attempt = context.finished(RunRecord::passed());
        assert_eq!(attempt.measures.get("markers"), Some(&4.0));
        assert_eq!(attempt.measures.get("members_only"), Some(&0.0));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_marker_is_a_whole_file_name() {
        let (root, listed) = written(
            "whole",
            &[
                ("a/Cargo.toml.bak", "[package]\n"),
                ("b/my-Cargo.toml", "[package]\n"),
                ("c/Cargo.toml/x", "x\n"),
                ("d/Cargo.toml", "[package]\n"),
            ],
        );
        let markers =
            performed(&mut context(), &root, &listed, &ecosystems()).expect("the markers are read");
        assert_eq!(markers, [marker("d/Cargo.toml", "rust", false)]);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_marker_that_only_lists_members_is_told_so_by_its_ecosystem_s_rule() {
        let (root, listed) = written(
            "members",
            &[
                ("ws/Cargo.toml", "[workspace]\nmembers = [\"a\"]\n"),
                ("ws/a/Cargo.toml", "[package]\nname = \"a\"\n"),
                (
                    "both/Cargo.toml",
                    "[workspace]\nmembers = [\"x\"]\n\n[package]\nname = \"both\"\n",
                ),
                (
                    "js/package.json",
                    "{\"private\": true, \"workspaces\": [\"packages/*\"]}",
                ),
                (
                    "js/packages/a/package.json",
                    "{\"name\": \"a\", \"main\": \"index.js\"}",
                ),
                (
                    "js-root-app/package.json",
                    "{\"workspaces\": [\"packages/*\"], \"bin\": \"cli.js\"}",
                ),
                (
                    "py/pyproject.toml",
                    "[tool.uv.workspace]\nmembers = [\"libs/*\"]\n",
                ),
                ("py/libs/a/pyproject.toml", "[project]\nname = \"a\"\n"),
                (
                    "py-app/pyproject.toml",
                    "[project]\nname = \"app\"\n\n[tool.uv.workspace]\nmembers = [\"libs/*\"]\n",
                ),
            ],
        );
        let mut context = context();
        let markers =
            performed(&mut context, &root, &listed, &ecosystems()).expect("the markers are read");
        assert_eq!(
            markers,
            [
                marker("both/Cargo.toml", "rust", false),
                marker("js/package.json", "typescript", true),
                marker("js/packages/a/package.json", "typescript", false),
                marker("js-root-app/package.json", "typescript", false),
                marker("py/libs/a/pyproject.toml", "python", false),
                marker("py/pyproject.toml", "python", true),
                marker("py-app/pyproject.toml", "python", false),
                marker("ws/Cargo.toml", "rust", true),
                marker("ws/a/Cargo.toml", "rust", false),
            ]
        );
        let attempt = context.finished(RunRecord::passed());
        assert_eq!(attempt.measures.get("markers"), Some(&9.0));
        assert_eq!(attempt.measures.get("members_only"), Some(&3.0));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_date_in_a_marker_does_not_stop_its_reading() {
        let (root, listed) = written(
            "dated",
            &[(
                "pyproject.toml",
                "[tool.uv.workspace]\nmembers = []\n\n[tool.release]\non = 2026-01-01\n",
            )],
        );
        let markers =
            performed(&mut context(), &root, &listed, &ecosystems()).expect("the markers are read");
        assert_eq!(markers, [marker("pyproject.toml", "python", true)]);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn an_ecosystem_with_no_rule_never_opens_its_marker() {
        let listed = vec![File {
            path: PathBuf::from("proto/buf.yaml"),
            bytes: 0,
            digest: "of a file that is not there".to_string(),
        }];
        let markers = performed(&mut context(), &env::temp_dir(), &listed, &ecosystems())
            .expect("nothing is opened");
        assert_eq!(markers, [marker("proto/buf.yaml", "proto", false)]);
    }

    #[test]
    fn two_ecosystems_sharing_a_marker_each_tell_the_file() {
        let (root, listed) = written("shared", &[("app/package.json", "{}")]);
        let both = vec![
            ecosystem("typescript", "package.json", None),
            ecosystem("javascript", "package.json", None),
        ];
        let markers =
            performed(&mut context(), &root, &listed, &both).expect("the markers are read");
        assert_eq!(
            markers,
            [
                marker("app/package.json", "javascript", false),
                marker("app/package.json", "typescript", false),
            ]
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_marker_changed_or_gone_since_it_was_listed_fails_the_attempt() {
        let (root, listed) = written("changed", &[("Cargo.toml", "[workspace]\n")]);
        fs::write(root.join("Cargo.toml"), "[package]\n").expect("the file is rewritten");
        let changed = performed(&mut context(), &root, &listed, &ecosystems())
            .expect_err("the list no longer tells this file");
        assert_eq!(changed, "Cargo.toml changed since it was listed");
        fs::remove_file(root.join("Cargo.toml")).expect("the file is removed");
        let gone = performed(&mut context(), &root, &listed, &ecosystems())
            .expect_err("the file is not there");
        assert!(gone.starts_with("Cargo.toml could not be read: "), "{gone}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_marker_that_does_not_read_as_its_format_fails_the_attempt() {
        let (root, listed) = written(
            "broken",
            &[("a/Cargo.toml", "[workspace\n"), ("b/package.json", "{")],
        );
        let rust = vec![ecosystem(
            "rust",
            "Cargo.toml",
            Some((&["workspace"], &["package"])),
        )];
        let toml =
            performed(&mut context(), &root, &listed, &rust).expect_err("the marker is not toml");
        assert!(
            toml.starts_with("a/Cargo.toml could not be read as toml: "),
            "{toml}"
        );
        assert!(!toml.contains('\n'), "the reason is one line: {toml}");
        let typescript = vec![ecosystem(
            "typescript",
            "package.json",
            Some((&["workspaces"], &[])),
        )];
        let json = performed(&mut context(), &root, &listed, &typescript)
            .expect_err("the marker is not json");
        assert!(
            json.starts_with("b/package.json could not be read as json: "),
            "{json}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_rule_on_a_marker_of_no_format_it_knows_fails_the_attempt() {
        let (root, listed) = written("format", &[("setup.cfg", "[metadata]\nname = x\n")]);
        let ruled = vec![ecosystem("legacy", "setup.cfg", Some((&["metadata"], &[])))];
        let failed = performed(&mut context(), &root, &listed, &ruled)
            .expect_err("no rule can be read from an ini file");
        assert_eq!(
            failed,
            "setup.cfg is neither toml, json nor yaml: no rule can be read from it"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_yaml_marker_is_read_by_its_keys_at_no_indent() {
        let (root, listed) = written(
            "yaml",
            &[
                ("a/buf.yaml", "version: v2\nmodules:\n  - path: proto\n"),
                (
                    "b/buf.yaml",
                    "version: v2\n# modules: none\nlint:\n  use:\n    - modules\n",
                ),
            ],
        );
        let ruled = vec![ecosystem("proto", "buf.yaml", Some((&["modules"], &[])))];
        let markers = performed(&mut context(), &root, &listed, &ruled).expect("both read");
        assert_eq!(
            markers,
            [
                marker("a/buf.yaml", "proto", true),
                marker("b/buf.yaml", "proto", false),
            ],
            "a key at no indent counts; a comment or a nested key does not"
        );
        let _ = fs::remove_dir_all(&root);
    }
}
