use task_runner::context::task_context::TaskContext;
use task_runner::declaration::pull::In;
use task_runner::declaration::task::Task;
use task_runner::record::task_failure::TaskFailure;

use crate::domain::planned_checks::PlannedChecks;
use crate::domain::subprocess::Subprocess;
use crate::domain::unit_check::UnitCheck;

pub fn list_check_subprocesses_task() -> Task {
    Task::new(
        "list_check_subprocesses",
        |_: &mut TaskContext, planned: In<PlannedChecks>| {
            Ok::<Vec<Subprocess>, TaskFailure>(list_check_subprocesses(&planned.checks))
        },
    )
    .line(
        "every program the checks run, in their order: each check's own, then the one printing \
         its root when it has one",
    )
}

fn list_check_subprocesses(checks: &[UnitCheck]) -> Vec<Subprocess> {
    checks
        .iter()
        .flat_map(UnitCheck::subprocesses)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::list_check_subprocesses;
    use crate::domain::ecosystem::Reader;
    use crate::domain::subprocess::Subprocess;
    use crate::domain::unit_check::CheckReading;
    use crate::domain::unit_check::UnitCheck;

    fn program(name: &str) -> Subprocess {
        Subprocess {
            name: name.to_string(),
            program: "cargo".to_string(),
            arguments: Vec::new(),
            folder: PathBuf::from("/repository"),
            configs: Vec::new(),
            cone: None,
        }
    }

    fn check(name: &str, reading: Option<CheckReading>) -> UnitCheck {
        UnitCheck {
            check: name.to_string(),
            ecosystem: "rust".to_string(),
            unit: PathBuf::new(),
            subprocess: program(name),
            reading,
        }
    }

    #[test]
    fn each_check_s_program_is_listed_in_order_with_the_one_printing_its_root_after_it() {
        let rooted = CheckReading {
            reader: Reader::CargoJson,
            root: Some(program("root of lint")),
        };
        let unrooted = CheckReading {
            reader: Reader::CargoJson,
            root: None,
        };
        let checks = [
            check("format", None),
            check("lint", Some(rooted)),
            check("test", Some(unrooted)),
        ];
        let names: Vec<String> = list_check_subprocesses(&checks)
            .into_iter()
            .map(|subprocess| subprocess.name)
            .collect();
        assert_eq!(names, ["format", "lint", "root of lint", "test"]);
    }
}
