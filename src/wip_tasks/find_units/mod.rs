mod gitignore;
mod group_files_by_unit;
mod list_files;
mod read_markers;

use crate::wip::pull::In;
use crate::wip::scope::Scope;
use crate::wip::task::Task;
use crate::wip::trace::outcome::Failure;
use crate::wip_domain::ecosystem::Ecosystem;
use crate::wip_domain::folder::Folder;
use crate::wip_domain::unit::Unit;
use crate::wip_tasks::find_units::group_files_by_unit::group_files_by_unit;
use crate::wip_tasks::find_units::list_files::list_files;
use crate::wip_tasks::find_units::read_markers::read_markers;

pub fn find_units_task() -> Task {
    Task::new(
        "find_units",
        |scope: &mut Scope, (folder, ecosystems): (In<Folder>, In<Vec<Ecosystem>>)| {
            find_units(scope, &folder, &ecosystems)
        },
    )
    .line(
        "each unit under one folder with its markers and its own files: the folder itself and \
         any folder under it holding an ecosystem's marker; what a .gitignore leaves out, \
         third_party, dot-folders, links and any folder below holding its own .arbiter are \
         never read",
    )
}

fn find_units(
    scope: &mut Scope,
    folder: &Folder,
    ecosystems: &[Ecosystem],
) -> Result<Vec<Unit>, Failure> {
    let files = scope.step("list_files", |step| list_files(step, folder))?;
    let markers = scope.step("read_markers", |step| {
        read_markers(step, folder, &files, ecosystems)
    })?;
    scope.step("group_files_by_unit", |step| {
        let units = group_files_by_unit(&files, &markers);
        step.measure("units", units.len() as f64);
        Ok(units)
    })
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::PathBuf;

    use super::find_units;
    use crate::wip::job::cancel::Cancel;
    use crate::wip::job::id::JobId;
    use crate::wip::job::id::RunId;
    use crate::wip::layout::Site;
    use crate::wip::scope::Scope;
    use crate::wip::trace::outcome::Outcome;
    use crate::wip_domain::ecosystem::Ecosystem;
    use crate::wip_domain::ecosystem::MembersOnly;
    use crate::wip_domain::folder::Folder;

    // A folder of its own per test, holding the files written.
    fn folder(name: &str, written: &[(&str, &str)]) -> PathBuf {
        let root = env::temp_dir().join(format!(
            "arbiter-units-{name}-{}-{}",
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

    // Python, whose marker is never opened, and Rust, whose marker is read
    // for the rule that tells one only listing members.
    fn ecosystems() -> Vec<Ecosystem> {
        vec![
            Ecosystem {
                name: "python".to_string(),
                marker: "pyproject.toml".to_string(),
                members_only: None,
                tools: Vec::new(),
                configs: Vec::new(),
            },
            Ecosystem {
                name: "rust".to_string(),
                marker: "Cargo.toml".to_string(),
                members_only: Some(MembersOnly {
                    has: vec!["workspace".to_string()],
                    lacks: vec!["package".to_string()],
                }),
                tools: Vec::new(),
                configs: Vec::new(),
            },
        ]
    }

    fn scope() -> Scope {
        let site = Site {
            dir: env::temp_dir(),
            store: env::temp_dir(),
        };
        Scope::new(
            JobId::new(RunId::next(), "find_units", 1),
            Cancel::new(),
            site,
        )
    }

    #[test]
    fn the_units_are_found_in_three_steps_each_leaving_its_own_record() {
        let root = folder(
            "found",
            &[
                ("Cargo.toml", "[package]\n"),
                ("src/lib.rs", "pub fn one() {}\n"),
                ("tools/probe/pyproject.toml", "[project]\n"),
                ("tools/probe/probe.py", "x = 1\n"),
            ],
        );
        let mut scope = scope();
        let units = find_units(&mut scope, &Folder { path: root.clone() }, &ecosystems())
            .expect("the units are found");
        let told: Vec<(String, usize)> = units
            .iter()
            .map(|unit| (unit.path.to_string_lossy().into_owned(), unit.files.len()))
            .collect();
        assert_eq!(told, [(String::new(), 2), ("tools/probe".to_string(), 2)]);
        let step = scope.finished("find_units", Outcome::Ok);
        let steps: Vec<(&str, &Outcome)> = step
            .children
            .iter()
            .map(|inner| (inner.name.as_str(), &inner.outcome))
            .collect();
        assert_eq!(
            steps,
            [
                ("list_files", &Outcome::Ok),
                ("read_markers", &Outcome::Ok),
                ("group_files_by_unit", &Outcome::Ok)
            ]
        );
        assert_eq!(step.children[0].measures.get("files"), Some(&4.0));
        assert_eq!(step.children[1].measures.get("markers"), Some(&2.0));
        assert_eq!(step.children[2].measures.get("units"), Some(&2.0));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_step_that_fails_fails_the_whole_and_no_later_step_is_taken() {
        let root = folder("broken", &[("Cargo.toml", "[package\n")]);
        let mut scope = scope();
        let failed = find_units(&mut scope, &Folder { path: root.clone() }, &ecosystems())
            .expect_err("a marker that does not read stops the search");
        assert!(
            failed
                .reason
                .contains("Cargo.toml could not be read as toml"),
            "the reason is the step's own: {}",
            failed.reason
        );
        let step = scope.finished("find_units", Outcome::Failed(failed.reason.clone()));
        let steps: Vec<(&str, bool)> = step
            .children
            .iter()
            .map(|inner| (inner.name.as_str(), inner.outcome == Outcome::Ok))
            .collect();
        assert_eq!(steps, [("list_files", true), ("read_markers", false)]);
        let _ = fs::remove_dir_all(&root);
    }
}
