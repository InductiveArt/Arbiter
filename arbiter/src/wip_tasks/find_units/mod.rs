mod list_files;
mod read_markers;

use task_runner::context::task_context::TaskContext;
use task_runner::declaration::pull::In;
use task_runner::declaration::task::Task;
use task_runner::record::task_failure::TaskFailure;

use crate::core::units::group_files_by_unit;
use crate::domain::ecosystem::Ecosystem;
use crate::domain::folder::Folder;
use crate::domain::found_units::FoundUnits;
use crate::wip_tasks::find_units::list_files::list_files;
use crate::wip_tasks::find_units::read_markers::read_markers;

pub fn find_units_task() -> Task {
    Task::new(
        "find_units",
        |context: &mut TaskContext, (folder, ecosystems): (In<Folder>, In<Vec<Ecosystem>>)| {
            find_units(context, &folder, &ecosystems)
        },
    )
    .line(
        "each unit under one folder with its markers and its own files: the folder itself and \
         any folder under it holding an ecosystem's marker; what a .gitignore leaves out unless \
         git carries it, the ecosystems' build output, third_party, dot-folders, links and \
         any folder below holding its own .arbiter are never read, only named",
    )
}

fn find_units(
    context: &mut TaskContext,
    folder: &Folder,
    ecosystems: &[Ecosystem],
) -> Result<FoundUnits, TaskFailure> {
    let listing = context.substep("list_files", |substep| list_files(substep, folder))?;
    let markers = context.substep("read_markers", |substep| {
        read_markers(substep, folder, &listing.files, ecosystems)
    })?;
    let units = context.substep("group_files_by_unit", |substep| {
        let units = group_files_by_unit(&listing.files, &markers);
        substep.measure("units", units.len() as f64);
        Ok(units)
    })?;
    Ok(FoundUnits {
        units,
        foreign: listing.foreign,
    })
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::PathBuf;

    use task_runner::context::stop_token::StopToken;
    use task_runner::context::task_context::TaskContext;
    use task_runner::contract::RunId;
    use task_runner::contract::RunRecord;
    use task_runner::contract::TaskAttemptId;

    use super::find_units;
    use crate::domain::ecosystem::Ecosystem;
    use crate::domain::ecosystem::MembersOnly;
    use crate::domain::folder::Folder;

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

    fn context() -> TaskContext {
        TaskContext::new(
            TaskAttemptId::new(RunId::next(), "find_units", 1),
            StopToken::new(),
            env::temp_dir(),
        )
    }

    #[test]
    fn the_units_are_found_in_three_substeps_each_leaving_its_own_record() {
        let root = folder(
            "found",
            &[
                ("Cargo.toml", "[package]\n"),
                ("src/lib.rs", "pub fn one() {}\n"),
                ("tools/probe/pyproject.toml", "[project]\n"),
                ("tools/probe/probe.py", "x = 1\n"),
            ],
        );
        let mut context = context();
        let found = find_units(&mut context, &Folder { path: root.clone() }, &ecosystems())
            .expect("the units are found");
        let told: Vec<(String, usize)> = found
            .units
            .iter()
            .map(|unit| (unit.path.to_string_lossy().into_owned(), unit.files.len()))
            .collect();
        assert_eq!(told, [(String::new(), 2), ("tools/probe".to_string(), 2)]);
        let attempt = context.finished(RunRecord::passed());
        let substeps: Vec<(&str, &RunRecord)> = attempt
            .substeps
            .iter()
            .map(|inner| (inner.name.as_str(), inner.outcome()))
            .collect();
        assert_eq!(
            substeps,
            [
                ("list_files", &RunRecord::passed()),
                ("read_markers", &RunRecord::passed()),
                ("group_files_by_unit", &RunRecord::passed())
            ]
        );
        assert_eq!(attempt.substeps[0].measures.get("files"), Some(&4.0));
        assert_eq!(attempt.substeps[1].measures.get("markers"), Some(&2.0));
        assert_eq!(attempt.substeps[2].measures.get("units"), Some(&2.0));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_substep_that_fails_fails_the_whole_and_no_later_substep_is_taken() {
        let root = folder("broken", &[("Cargo.toml", "[package\n")]);
        let mut context = context();
        let failed = find_units(&mut context, &Folder { path: root.clone() }, &ecosystems())
            .expect_err("a marker that does not read stops the search");
        assert!(
            failed
                .reason
                .contains("Cargo.toml could not be read as toml"),
            "the reason is the substep's own: {}",
            failed.reason
        );
        let attempt = context.finished(RunRecord::failed(failed.reason.clone()));
        let substeps: Vec<(&str, bool)> = attempt
            .substeps
            .iter()
            .map(|inner| (inner.name.as_str(), *inner.outcome() == RunRecord::passed()))
            .collect();
        assert_eq!(substeps, [("list_files", true), ("read_markers", false)]);
        let _ = fs::remove_dir_all(&root);
    }
}
