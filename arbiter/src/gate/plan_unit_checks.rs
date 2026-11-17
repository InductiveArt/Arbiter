use std::collections::BTreeMap;
use std::path::Path;

use task_runner::context::task_context::TaskContext;
use task_runner::declaration::pull::In;
use task_runner::declaration::task::Task;
use task_runner::record::task_failure::TaskFailure;

use crate::core::repository::is_test;
use crate::domain::ecosystem::Ecosystem;
use crate::domain::folder::Folder;
use crate::domain::found_units::FoundUnits;
use crate::domain::planned_checks::PlannedChecks;
use crate::domain::tool_run::ToolRun;
use crate::domain::unit::shown;
use crate::domain::unit::Unit;
use crate::domain::unit_check::CheckReading;
use crate::domain::unit_check::UnitCheck;
use crate::domain::unit_cone::UnitCone;

// What planning reads: the listed folder, the units found in it, the
// ecosystems, and each unit's input cone.
type PlanReads = (
    In<Folder>,
    In<FoundUnits>,
    In<Vec<Ecosystem>>,
    In<Vec<UnitCone>>,
);

pub fn plan_unit_checks_task() -> Task {
    Task::new(
        "plan_unit_checks",
        |context: &mut TaskContext, (folder, found, ecosystems, cones): PlanReads| {
            plan_unit_checks(context, &folder, &found.units, &ecosystems, &cones)
        },
    )
    .line(
        "each tool of each ecosystem once in every unit of that ecosystem, except a fixture and a \
         unit whose marker only lists members, as the program running it there, handed the \
         configs its words point at and the unit's input cone, with the reader of what it \
         prints and the program printing its root when it has them, and a note for each unit \
         an ecosystem plans nothing in; it starts nothing",
    )
}

// Ecosystems in the order given, the tools of each in the order its record
// writes them, each tool once per unit. A unit on a test path is a fixture:
// data for some test, not code to check.
fn plan_unit_checks(
    context: &mut TaskContext,
    folder: &Folder,
    units: &[Unit],
    ecosystems: &[Ecosystem],
    cones: &[UnitCone],
) -> Result<PlannedChecks, TaskFailure> {
    let cones: BTreeMap<&Path, &str> = cones
        .iter()
        .map(|cone| (cone.unit.as_path(), cone.digest.as_str()))
        .collect();
    let mut checks = Vec::new();
    for ecosystem in ecosystems {
        let held: Vec<&Unit> = units
            .iter()
            .filter(|unit| !is_test(&unit.path))
            .filter(|unit| {
                unit.markers
                    .iter()
                    .any(|marker| marker.ecosystem == ecosystem.name && !marker.members_only)
            })
            .collect();
        for tool in &ecosystem.tools {
            // A tool whose findings are read is run by the words that print them.
            let run = tool
                .findings
                .as_ref()
                .map_or(&tool.run, |readable| &readable.run);
            let Some(words) = ToolRun::of(run, ecosystem) else {
                return Err(TaskFailure::from(format!(
                    "the tool {} names no program",
                    tool.name
                )));
            };
            let root = match tool
                .findings
                .as_ref()
                .and_then(|readable| readable.root.as_ref())
            {
                None => None,
                Some(root) => Some(ToolRun::of(root, ecosystem).ok_or_else(|| {
                    TaskFailure::from(format!(
                        "the root of the tool {} names no program",
                        tool.name
                    ))
                })?),
            };
            for unit in &held {
                let Some(cone) = cones.get(unit.path.as_path()) else {
                    return Err(TaskFailure::from(format!(
                        "the unit {} has no input cone",
                        shown(&unit.path)
                    )));
                };
                let at = folder.path.join(&unit.path);
                let name = format!("{} in {}", tool.name, shown(&unit.path));
                let reading = tool.findings.as_ref().map(|readable| CheckReading {
                    reader: readable.reader,
                    root: root
                        .as_ref()
                        .map(|root| root.subprocess(format!("root of {name}"), &at, Some(cone))),
                });
                checks.push(UnitCheck {
                    check: tool.name.clone(),
                    ecosystem: ecosystem.name.clone(),
                    unit: unit.path.clone(),
                    subprocess: words.subprocess(name, &at, Some(cone)),
                    reading,
                });
            }
        }
    }
    context.measure("checks", checks.len() as f64);
    Ok(PlannedChecks {
        checks,
        notes: unplanned(units, ecosystems),
    })
}

// A note for each unit an ecosystem holds and plans nothing in, unit by unit:
// a marker only listing members is checked through its members, and an
// ecosystem with no tool declares none yet.
fn unplanned(units: &[Unit], ecosystems: &[Ecosystem]) -> Vec<String> {
    let mut notes = Vec::new();
    for unit in units.iter().filter(|unit| !is_test(&unit.path)) {
        for ecosystem in ecosystems {
            let Some(marker) = unit
                .markers
                .iter()
                .find(|marker| marker.ecosystem == ecosystem.name)
            else {
                continue;
            };
            if marker.members_only {
                notes.push(format!(
                    "{} is a {} workspace; its members are the units checked",
                    shown(&unit.path),
                    ecosystem.name
                ));
            } else if ecosystem.tools.is_empty() {
                notes.push(format!(
                    "{} is {}, which declares no tools yet",
                    shown(&unit.path),
                    ecosystem.name
                ));
            }
        }
    }
    notes
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::path::Path;
    use std::path::PathBuf;

    use task_runner::context::stop_token::StopToken;
    use task_runner::context::task_context::TaskContext;
    use task_runner::contract::RunId;
    use task_runner::contract::RunRecord;
    use task_runner::contract::TaskAttemptId;

    use super::plan_unit_checks;
    use crate::domain::ecosystem::Config;
    use crate::domain::ecosystem::Ecosystem;
    use crate::domain::ecosystem::ReadableRun;
    use crate::domain::ecosystem::Reader;
    use crate::domain::ecosystem::Tool;
    use crate::domain::folder::Folder;
    use crate::domain::marker::Marker;
    use crate::domain::subprocess::Subprocess;
    use crate::domain::unit::Unit;
    use crate::domain::unit_check::UnitCheck;
    use crate::domain::unit_cone::UnitCone;

    // Planning writes nothing, so no folder of its own is needed.
    fn context() -> TaskContext {
        TaskContext::new(
            TaskAttemptId::new(RunId::next(), "plan_unit_checks", 1),
            StopToken::new(),
            env::temp_dir(),
        )
    }

    fn folder() -> Folder {
        Folder {
            path: PathBuf::from("/repository"),
        }
    }

    // Each unit by its path, with the ecosystems marking it and whether each
    // marker only lists members.
    fn units(told: &[(&str, &[(&str, bool)])]) -> Vec<Unit> {
        told.iter()
            .map(|(path, markers)| Unit {
                path: PathBuf::from(path),
                markers: markers
                    .iter()
                    .map(|(ecosystem, members_only)| Marker {
                        path: PathBuf::from(path).join("marker"),
                        ecosystem: ecosystem.to_string(),
                        members_only: *members_only,
                    })
                    .collect(),
                files: Vec::new(),
            })
            .collect()
    }

    // A cone for each unit, its digest naming the unit.
    fn cones(units: &[Unit]) -> Vec<UnitCone> {
        units
            .iter()
            .map(|unit| UnitCone {
                unit: unit.path.clone(),
                digest: format!("cone of {}", unit.path.display()),
            })
            .collect()
    }

    fn planned(units: &[Unit], ecosystems: &[Ecosystem]) -> Vec<UnitCheck> {
        plan_unit_checks(&mut context(), &folder(), units, ecosystems, &cones(units))
            .expect("the checks are planned")
            .checks
    }

    fn ecosystem(name: &str, tools: &[(&str, &[&str])], configs: &[(&str, &str)]) -> Ecosystem {
        Ecosystem {
            name: name.to_string(),
            marker: "marker".to_string(),
            members_only: None,
            tools: tools
                .iter()
                .map(|(name, run)| Tool {
                    name: name.to_string(),
                    run: words(run),
                    fix: None,
                    findings: None,
                })
                .collect(),
            configs: configs
                .iter()
                .map(|(name, content)| Config {
                    name: name.to_string(),
                    content: content.to_string(),
                })
                .collect(),
        }
    }

    fn words(told: &[&str]) -> Vec<String> {
        told.iter().map(|word| word.to_string()).collect()
    }

    // Each check's program by its name, its folder and its whole line.
    fn told(checks: &[UnitCheck]) -> Vec<(String, String, String)> {
        checks
            .iter()
            .map(|check| line_of(&check.subprocess))
            .collect()
    }

    fn line_of(subprocess: &Subprocess) -> (String, String, String) {
        let mut line = vec![subprocess.program.clone()];
        line.extend(subprocess.arguments.iter().cloned());
        (
            subprocess.name.clone(),
            subprocess.folder.to_string_lossy().into_owned(),
            line.join(" "),
        )
    }

    fn line(name: &str, folder: &str, words: &str) -> (String, String, String) {
        (name.to_string(), folder.to_string(), words.to_string())
    }

    #[test]
    fn each_tool_is_planned_once_per_unit_of_its_ecosystem_in_the_order_written() {
        let ecosystems = vec![
            ecosystem(
                "rust",
                &[
                    ("format_rust", &["cargo", "fmt", "--check"]),
                    ("lint_rust", &["cargo", "clippy"]),
                ],
                &[],
            ),
            ecosystem("python", &[("format_python", &["ruff", "format"])], &[]),
        ];
        let units = units(&[
            ("", &[("rust", false)]),
            ("libs", &[("rust", true)]),
            ("libs/a", &[("python", false), ("rust", false)]),
            ("tools/probe", &[("python", false)]),
        ]);
        let mut context = context();
        let checks = plan_unit_checks(&mut context, &folder(), &units, &ecosystems, &cones(&units))
            .expect("the checks are planned")
            .checks;
        assert_eq!(
            told(&checks),
            [
                line("format_rust in .", "/repository/", "cargo fmt --check"),
                line(
                    "format_rust in libs/a",
                    "/repository/libs/a",
                    "cargo fmt --check"
                ),
                line("lint_rust in .", "/repository/", "cargo clippy"),
                line("lint_rust in libs/a", "/repository/libs/a", "cargo clippy"),
                line(
                    "format_python in libs/a",
                    "/repository/libs/a",
                    "ruff format"
                ),
                line(
                    "format_python in tools/probe",
                    "/repository/tools/probe",
                    "ruff format"
                ),
            ]
        );
        assert_eq!(
            checks
                .iter()
                .map(|check| (check.check.as_str(), check.unit.as_path()))
                .collect::<Vec<_>>()[..2],
            [
                ("format_rust", Path::new("")),
                ("format_rust", Path::new("libs/a"))
            ],
            "each check names its tool and its unit"
        );
        assert!(
            checks.iter().all(|check| check.reading.is_none()),
            "a tool whose findings are not read has no reader and asks no root"
        );
        let recorded = context.finished(RunRecord::passed());
        assert!(
            recorded.resources().processes.is_empty(),
            "planning starts no program"
        );
        assert_eq!(recorded.measures.get("checks"), Some(&6.0));
    }

    #[test]
    fn a_tool_whose_findings_are_read_is_run_by_the_words_that_print_them_beside_its_root() {
        let mut rust = ecosystem("rust", &[("lint_rust", &["cargo", "clippy"])], &[]);
        rust.tools[0].findings = Some(ReadableRun {
            reader: Reader::CargoJson,
            run: words(&["cargo", "clippy", "--message-format=json"]),
            root: Some(words(&["cargo", "locate-project", "--workspace"])),
        });
        let checks = planned(&units(&[("libs/a", &[("rust", false)])]), &[rust]);
        assert_eq!(
            told(&checks),
            [line(
                "lint_rust in libs/a",
                "/repository/libs/a",
                "cargo clippy --message-format=json"
            )]
        );
        let reading = checks[0].reading.as_ref().expect("its findings are read");
        assert_eq!(reading.reader, Reader::CargoJson);
        let root = reading.root.as_ref().expect("its root is asked");
        assert_eq!(
            line_of(root),
            line(
                "root of lint_rust in libs/a",
                "/repository/libs/a",
                "cargo locate-project --workspace"
            ),
            "the root is asked in the folder the tool runs in"
        );
        assert_eq!(root.cone, checks[0].subprocess.cone, "over the same files");
    }

    #[test]
    fn each_program_carries_the_cone_of_its_unit_and_a_fixture_is_never_checked() {
        let ecosystems = vec![ecosystem(
            "rust",
            &[("lint_rust", &["cargo", "clippy"])],
            &[],
        )];
        let checks = planned(
            &units(&[
                ("", &[("rust", false)]),
                ("libs/a", &[("rust", false)]),
                ("tests/fixture", &[("rust", false)]),
            ]),
            &ecosystems,
        );
        assert_eq!(
            checks
                .iter()
                .map(|check| (
                    check.subprocess.name.as_str(),
                    check.subprocess.cone.as_deref()
                ))
                .collect::<Vec<_>>(),
            [
                ("lint_rust in .", Some("cone of ")),
                ("lint_rust in libs/a", Some("cone of libs/a"))
            ]
        );
    }

    #[test]
    fn a_unit_an_ecosystem_plans_nothing_in_is_noted_unit_by_unit_and_a_fixture_never() {
        let ecosystems = vec![
            ecosystem("proto", &[], &[]),
            ecosystem("rust", &[("lint_rust", &["cargo", "clippy"])], &[]),
        ];
        let units = units(&[
            ("", &[("rust", true)]),
            ("contracts", &[("proto", false)]),
            ("tests/fixture", &[("rust", true)]),
        ]);
        let planned = plan_unit_checks(
            &mut context(),
            &folder(),
            &units,
            &ecosystems,
            &cones(&units),
        )
        .expect("the checks are planned");
        assert!(planned.checks.is_empty());
        assert_eq!(
            planned.notes,
            [
                ". is a rust workspace; its members are the units checked",
                "contracts is proto, which declares no tools yet"
            ]
        );
    }

    #[test]
    fn a_unit_with_no_input_cone_fails_the_plan() {
        let ecosystems = vec![ecosystem(
            "rust",
            &[("lint_rust", &["cargo", "clippy"])],
            &[],
        )];
        let units = units(&[("libs/a", &[("rust", false)])]);
        let failed = plan_unit_checks(&mut context(), &folder(), &units, &ecosystems, &[])
            .expect_err("no cone to stand on");
        assert_eq!(failed.reason, "the unit libs/a has no input cone");
    }

    #[test]
    fn a_program_whose_words_point_at_the_configs_is_handed_them_and_keeps_the_word_for_its_run() {
        let ecosystems = vec![ecosystem(
            "python",
            &[
                (
                    "lint_python",
                    &["ruff", "check", "--config", "{pack}/ruff.toml"],
                ),
                ("format_python", &["ruff", "format"]),
            ],
            &[("ruff.toml", "line-length = 100\n")],
        )];
        let checks = planned(&units(&[("", &[("python", false)])]), &ecosystems);
        assert_eq!(
            checks[0].subprocess.arguments,
            ["check", "--config", "{pack}/ruff.toml"]
        );
        assert_eq!(
            checks[0].subprocess.configs,
            [Config {
                name: "ruff.toml".to_string(),
                content: "line-length = 100\n".to_string(),
            }]
        );
        assert!(
            checks[1].subprocess.configs.is_empty(),
            "words that never point at the configs are handed none"
        );
    }

    #[test]
    fn an_ecosystem_with_no_unit_plans_nothing() {
        let ecosystems = vec![ecosystem(
            "python",
            &[(
                "lint_python",
                &["ruff", "check", "--config", "{pack}/ruff.toml"],
            )],
            &[("ruff.toml", "line-length = 100\n")],
        )];
        let checks = planned(
            &units(&[("", &[("rust", false)]), ("libs", &[("python", true)])]),
            &ecosystems,
        );
        assert!(checks.is_empty());
    }

    #[test]
    fn a_tool_or_a_root_naming_no_program_fails_even_with_no_unit_to_run_it_in() {
        let ecosystems = vec![ecosystem("rust", &[("format_rust", &[])], &[])];
        let failed = plan_unit_checks(&mut context(), &folder(), &units(&[]), &ecosystems, &[])
            .expect_err("nothing to start");
        assert_eq!(failed.reason, "the tool format_rust names no program");
        let mut rootless = ecosystem("rust", &[("lint_rust", &["cargo"])], &[]);
        rootless.tools[0].findings = Some(ReadableRun {
            reader: Reader::CargoJson,
            run: words(&["cargo", "clippy"]),
            root: Some(Vec::new()),
        });
        let failed = plan_unit_checks(&mut context(), &folder(), &units(&[]), &[rootless], &[])
            .expect_err("no root to start");
        assert_eq!(
            failed.reason,
            "the root of the tool lint_rust names no program"
        );
    }
}
