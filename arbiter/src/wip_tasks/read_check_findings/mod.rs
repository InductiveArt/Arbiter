mod read_cargo_json;

use std::fs;
use std::path::Path;
use std::path::PathBuf;

use task_runner::context::task_context::TaskContext;
use task_runner::declaration::pull::In;
use task_runner::declaration::task::Task;
use task_runner::record::task_failure::TaskFailure;

use crate::domain::diagnostic::Diagnostic;
use crate::domain::ecosystem::Reader;
use crate::domain::finding::Finding;
use crate::domain::folder::Folder;
use crate::domain::subprocess::SubprocessExit;
use crate::domain::unit_check::UnitCheck;
use crate::wip_tasks::read_check_findings::read_cargo_json::read_cargo_json;
use crate::wip_tasks::read_check_findings::read_cargo_json::workspace_root;

pub fn read_check_findings_task() -> Task {
    Task::new(
        "read_check_findings",
        |context: &mut TaskContext,
         (folder, checks, exits): (
            In<Folder>,
            In<Vec<UnitCheck>>,
            In<Vec<SubprocessExit>>,
        )| { read_check_findings(context, &folder, &checks, &exits) },
    )
    .line(
        "what each check printed, read into findings when it has a reader, their files placed \
         from the root printed for them; a check that did not exit with 0 and leaves no finding \
         is one finding, and every check is read even after one could not be",
    )
}

// The exits come as the checks list their programs. Every check is read even
// after one could not be, so one attempt tells each that could not.
fn read_check_findings(
    context: &mut TaskContext,
    folder: &Folder,
    checks: &[UnitCheck],
    exits: &[SubprocessExit],
) -> Result<Vec<Finding>, TaskFailure> {
    let listed: usize = checks.iter().map(|check| check.subprocesses().len()).sum();
    if exits.len() != listed {
        return Err(TaskFailure::from(format!(
            "the checks list {listed} programs, and the exits of {} were handed",
            exits.len()
        )));
    }
    let mut exits = exits.iter();
    let mut findings = Vec::new();
    let mut unread: Vec<&str> = Vec::new();
    for check in checks {
        let ran: Vec<&SubprocessExit> = exits.by_ref().take(check.subprocesses().len()).collect();
        match read_check(folder, check, &ran) {
            Ok(found) => findings.extend(found),
            Err(failure) => {
                context.note(format!("{}: {}", check.subprocess.name, failure.reason));
                unread.push(&check.subprocess.name);
            }
        }
    }
    context.measure("findings", findings.len() as f64);
    if unread.is_empty() {
        Ok(findings)
    } else {
        Err(TaskFailure::from(format!(
            "{} of {} checks could not be read: {}",
            unread.len(),
            checks.len(),
            unread.join(", ")
        )))
    }
}

// One check read from the exits of its programs: its own, then its root's.
// The root is read only when a finding needs placing: a tool that could not
// read its own project prints nothing to place, and its root fails the same
// way.
fn read_check(
    folder: &Folder,
    check: &UnitCheck,
    ran: &[&SubprocessExit],
) -> Result<Vec<Finding>, TaskFailure> {
    let Some((exit, rest)) = ran.split_first() else {
        return Err(TaskFailure::from("no exit was handed for it"));
    };
    let mut found = match &check.reading {
        None => Vec::new(),
        Some(reading) => {
            let diagnostics = match reading.reader {
                Reader::CargoJson => read_cargo_json(&exit.stdout),
            };
            if diagnostics.is_empty() {
                Vec::new()
            } else {
                let base = match (&reading.root, rest.first()) {
                    (None, _) => check.subprocess.folder.clone(),
                    (Some(root), Some(located)) => {
                        if located.code != Some(0) {
                            return Err(TaskFailure::from(format!(
                                "no root for its paths: {}",
                                located.ended(&root.program)
                            )));
                        }
                        match reading.reader {
                            Reader::CargoJson => workspace_root(&located.stdout)?,
                        }
                    }
                    (Some(_), None) => {
                        return Err(TaskFailure::from("no exit was handed for its root"));
                    }
                };
                placed(&check.check, diagnostics, &base, &folder.path)?
            }
        }
    };
    if exit.code != Some(0) && found.is_empty() {
        found.push(Finding {
            check: check.check.clone(),
            rule: None,
            file: check.unit.clone(),
            line: None,
            reason: format!(
                "{}; all it printed is kept in the record of {}",
                exit.ended(&check.subprocess.program),
                check.subprocess.name
            ),
        });
    }
    Ok(found)
}

// Each diagnostic as a finding of the check, its file by path from the listed
// folder when it is under it and by its whole path when it is not. Both
// folders are taken as the disk resolves them, so a link in the way of either
// does not move a file out of its place.
fn placed(
    check: &str,
    diagnostics: Vec<Diagnostic>,
    base: &Path,
    listed: &Path,
) -> Result<Vec<Finding>, TaskFailure> {
    let base = resolved(base)?;
    let listed = resolved(listed)?;
    Ok(diagnostics
        .into_iter()
        .map(|diagnostic| {
            let whole = base.join(&diagnostic.file);
            Finding {
                check: check.to_string(),
                rule: diagnostic.code,
                file: whole
                    .strip_prefix(&listed)
                    .map_or_else(|_| whole.clone(), Path::to_path_buf),
                line: diagnostic.line,
                reason: diagnostic.message,
            }
        })
        .collect())
}

fn resolved(folder: &Path) -> Result<PathBuf, TaskFailure> {
    fs::canonicalize(folder).map_err(|error| {
        TaskFailure::from(format!(
            "{} could not be resolved: {error}",
            folder.display()
        ))
    })
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

    use super::placed;
    use super::read_check_findings;
    use crate::domain::diagnostic::Diagnostic;
    use crate::domain::ecosystem::Reader;
    use crate::domain::folder::Folder;
    use crate::domain::subprocess::Subprocess;
    use crate::domain::subprocess::SubprocessExit;
    use crate::domain::unit_check::CheckReading;
    use crate::domain::unit_check::UnitCheck;

    const LINT: &str = r#"{"reason":"compiler-message","message":{"code":{"code":"clippy::eq_op"},"level":"error","message":"equal expressions as operands to `==`","spans":[{"file_name":"src/lib.rs","is_primary":true,"line_start":2}]}}"#;

    // A folder of its own per test, as the disk resolves it.
    fn listed(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!(
            "arbiter-findings-{name}-{}-{}",
            std::process::id(),
            RunId::next()
        ));
        fs::create_dir_all(&dir).expect("the folder is made");
        fs::canonicalize(&dir).expect("the folder resolves")
    }

    fn context() -> TaskContext {
        TaskContext::new(
            TaskAttemptId::new(RunId::next(), "read_check_findings", 1),
            StopToken::new(),
            env::temp_dir(),
        )
    }

    fn program(name: &str, folder: &Path) -> Subprocess {
        Subprocess {
            name: name.to_string(),
            program: "cargo".to_string(),
            arguments: Vec::new(),
            folder: folder.to_path_buf(),
        }
    }

    // The lint of one unit under the listed folder, its findings read as
    // cargo prints them, beside the program printing its root.
    fn lint(unit: &str, listed: &Path) -> UnitCheck {
        let name = format!("lint_rust in {unit}");
        let at = listed.join(unit);
        UnitCheck {
            check: "lint_rust".to_string(),
            unit: PathBuf::from(unit),
            subprocess: program(&name, &at),
            reading: Some(CheckReading {
                reader: Reader::CargoJson,
                root: Some(program(&format!("root of {name}"), &at)),
            }),
        }
    }

    fn exit(code: i32, stdout: &str) -> SubprocessExit {
        SubprocessExit {
            code: Some(code),
            signal: None,
            stdout: stdout.to_string(),
            stderr: String::new(),
        }
    }

    #[test]
    fn a_root_is_read_only_when_a_finding_needs_placing_and_the_finding_s_file_runs_from_it() {
        let listed = listed("rooted");
        fs::create_dir_all(listed.join("libs").join("b")).expect("the folder is made");
        let checks = [lint("a", &listed), lint("libs/b", &listed)];
        let root = format!("{{\"root\":\"{}\"}}", listed.join("Cargo.toml").display());
        // The first check prints nothing to place, so its failed root is never read.
        let exits = [exit(0, ""), exit(101, ""), exit(101, LINT), exit(0, &root)];
        let found = read_check_findings(
            &mut context(),
            &Folder {
                path: listed.clone(),
            },
            &checks,
            &exits,
        )
        .expect("both checks are read");
        let told: Vec<(&str, &Path, Option<u32>)> = found
            .iter()
            .map(|finding| (finding.check.as_str(), finding.file.as_path(), finding.line))
            .collect();
        assert_eq!(
            told,
            [("lint_rust", Path::new("src/lib.rs"), Some(2))],
            "the file runs from the root cargo named, not from the unit"
        );
        let _ = fs::remove_dir_all(&listed);
    }

    #[test]
    fn a_finding_with_no_root_to_place_it_fails_its_check_once_every_check_is_read() {
        let listed = listed("rootless");
        let checks = [lint("a", &listed), lint("b", &listed)];
        let exits = [
            exit(101, LINT),
            exit(101, ""),
            exit(101, LINT),
            exit(101, ""),
        ];
        let mut context = context();
        let failed = read_check_findings(
            &mut context,
            &Folder {
                path: listed.clone(),
            },
            &checks,
            &exits,
        )
        .expect_err("no finding can be placed");
        assert_eq!(
            failed.reason,
            "2 of 2 checks could not be read: lint_rust in a, lint_rust in b"
        );
        let attempt = context.finished(RunRecord::failed(failed.reason.clone()));
        assert_eq!(
            attempt.evidence,
            [
                "lint_rust in a: no root for its paths: cargo exited with 101",
                "lint_rust in b: no root for its paths: cargo exited with 101"
            ]
        );
        let _ = fs::remove_dir_all(&listed);
    }

    #[test]
    fn exits_that_do_not_match_the_programs_the_checks_list_fail_the_task() {
        let listed = listed("unmatched");
        let failed = read_check_findings(
            &mut context(),
            &Folder {
                path: listed.clone(),
            },
            &[lint("a", &listed)],
            &[exit(0, "")],
        )
        .expect_err("the root's exit is missing");
        assert_eq!(
            failed.reason,
            "the checks list 2 programs, and the exits of 1 were handed"
        );
        let _ = fs::remove_dir_all(&listed);
    }

    #[test]
    fn a_file_under_the_listed_folder_is_told_from_it_even_through_a_link_and_one_outside_whole() {
        let attempt = env::temp_dir().join(format!(
            "arbiter-findings-placed-{}-{}",
            std::process::id(),
            RunId::next()
        ));
        let member = attempt.join("libs").join("a");
        fs::create_dir_all(&member).expect("the folder is made");
        // The base as the disk resolves it, the listed folder as written: on a
        // system whose temporary folder sits behind a link the two differ.
        let base = fs::canonicalize(&attempt).expect("the folder resolves");
        let diagnostic = |file: &str| Diagnostic {
            file: PathBuf::from(file),
            line: Some(2),
            code: Some("clippy::eq_op".to_string()),
            message: "told".to_string(),
        };
        let found = placed(
            "lint_rust",
            vec![
                diagnostic("libs/a/src/lib.rs"),
                diagnostic("/elsewhere/x.rs"),
            ],
            &base,
            &attempt,
        )
        .expect("both folders resolve");
        let paths: Vec<&Path> = found.iter().map(|finding| finding.file.as_path()).collect();
        assert_eq!(
            paths,
            [Path::new("libs/a/src/lib.rs"), Path::new("/elsewhere/x.rs")]
        );
        assert_eq!(found[0].check, "lint_rust");
        assert_eq!(found[0].rule.as_deref(), Some("clippy::eq_op"));
        let _ = fs::remove_dir_all(&attempt);
    }
}
