use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::Path;

use task_runner::context::task_context::TaskContext;
use task_runner::declaration::pull::In;
use task_runner::declaration::task::Task;
use task_runner::record::task_failure::TaskFailure;

use crate::domain::ecosystem::Ecosystem;
use crate::domain::folder::Folder;
use crate::domain::subprocess::Subprocess;
use crate::domain::unit::Unit;
use crate::domain::unit_check::CheckReading;
use crate::domain::unit_check::UnitCheck;

pub fn plan_unit_checks_task() -> Task {
    Task::new(
        "plan_unit_checks",
        |context: &mut TaskContext,
         (folder, units, ecosystems): (In<Folder>, In<Vec<Unit>>, In<Vec<Ecosystem>>)| {
            plan_unit_checks(context, &folder, &units, &ecosystems)
        },
    )
    .line(
        "each tool of each ecosystem once in every unit of that ecosystem, except a unit whose \
         marker only lists members, as the program running it there, with the reader of what \
         it prints and the program printing its root when it has them; it starts nothing, and \
         writes the configs the programs point at in its own folder",
    )
}

// Ecosystems in the order given, the tools of each in the order its record
// writes them, each tool once per unit.
fn plan_unit_checks(
    context: &mut TaskContext,
    folder: &Folder,
    units: &[Unit],
    ecosystems: &[Ecosystem],
) -> Result<Vec<UnitCheck>, TaskFailure> {
    let mut checks = Vec::new();
    for ecosystem in ecosystems {
        let held: Vec<&Unit> = units
            .iter()
            .filter(|unit| {
                unit.markers
                    .iter()
                    .any(|marker| marker.ecosystem == ecosystem.name && !marker.members_only)
            })
            .collect();
        let configs = context.dir().join("config").join(&ecosystem.name);
        let pointed = configs.to_string_lossy();
        for tool in &ecosystem.tools {
            // A tool whose findings are read is run by the words that print them.
            let run = tool
                .findings
                .as_ref()
                .map_or(&tool.run, |readable| &readable.run);
            let Some((program, arguments)) = split(run, &pointed) else {
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
                Some(words) => Some(split(words, &pointed).ok_or_else(|| {
                    TaskFailure::from(format!(
                        "the root of the tool {} names no program",
                        tool.name
                    ))
                })?),
            };
            for unit in &held {
                let at = folder.path.join(&unit.path);
                let name = format!("{} in {}", tool.name, shown(unit));
                let reading = tool.findings.as_ref().map(|readable| CheckReading {
                    reader: readable.reader,
                    root: root.as_ref().map(|(program, arguments)| Subprocess {
                        name: format!("root of {name}"),
                        program: program.clone(),
                        arguments: arguments.clone(),
                        folder: at.clone(),
                    }),
                });
                checks.push(UnitCheck {
                    check: tool.name.clone(),
                    unit: unit.path.clone(),
                    subprocess: Subprocess {
                        name,
                        program: program.clone(),
                        arguments: arguments.clone(),
                        folder: at,
                    },
                    reading,
                });
            }
        }
        if !held.is_empty() {
            write_configs(&configs, ecosystem)?;
        }
    }
    context.measure("checks", checks.len() as f64);
    Ok(checks)
}

// The words with `{pack}` given the config folder, as the program and the
// arguments handed to it.
fn split(words: &[String], pointed: &str) -> Option<(String, Vec<String>)> {
    let mut given = words.iter().map(|word| word.replace("{pack}", pointed));
    let program = given.next()?;
    Some((program, given.collect()))
}

// The ecosystem's config files, written inside the attempt's own folder so
// nothing changes in the folder the tools are run on.
fn write_configs(into: &Path, ecosystem: &Ecosystem) -> Result<(), TaskFailure> {
    if ecosystem.configs.is_empty() {
        return Ok(());
    }
    if !plain(&ecosystem.name) {
        return Err(TaskFailure::from(format!(
            "the ecosystem name {} is not a plain file name",
            ecosystem.name
        )));
    }
    fs::create_dir_all(into).map_err(|error| unwritten(into, &error))?;
    for config in &ecosystem.configs {
        if !plain(&config.name) {
            return Err(TaskFailure::from(format!(
                "the config {} of {} is not a plain file name",
                config.name, ecosystem.name
            )));
        }
        let file = into.join(&config.name);
        fs::write(&file, &config.content).map_err(|error| unwritten(&file, &error))?;
    }
    Ok(())
}

// A name that stays inside the folder it is joined to.
fn plain(name: &str) -> bool {
    Path::new(name).file_name().and_then(OsStr::to_str) == Some(name)
}

fn shown(unit: &Unit) -> String {
    if unit.path.as_os_str().is_empty() {
        ".".to_string()
    } else {
        unit.path.to_string_lossy().into_owned()
    }
}

fn unwritten(path: &Path, error: &io::Error) -> TaskFailure {
    TaskFailure::from(format!("{} could not be written: {error}", path.display()))
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

    // The attempt's own folder, one per test.
    fn attempt(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!(
            "arbiter-checks-{name}-{}-{}",
            std::process::id(),
            RunId::next()
        ));
        fs::create_dir_all(&dir).expect("the folder is made");
        dir
    }

    fn context(attempt: &Path) -> TaskContext {
        TaskContext::new(
            TaskAttemptId::new(RunId::next(), "plan_unit_checks", 1),
            StopToken::new(),
            attempt.to_path_buf(),
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
        let attempt = attempt("order");
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
        let mut context = context(&attempt);
        let checks = plan_unit_checks(&mut context, &folder(), &units, &ecosystems)
            .expect("the checks are planned");
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
        let _ = fs::remove_dir_all(&attempt);
    }

    #[test]
    fn a_tool_whose_findings_are_read_is_run_by_the_words_that_print_them_beside_its_root() {
        let attempt = attempt("readable");
        let mut rust = ecosystem("rust", &[("lint_rust", &["cargo", "clippy"])], &[]);
        rust.tools[0].findings = Some(ReadableRun {
            reader: Reader::CargoJson,
            run: words(&["cargo", "clippy", "--message-format=json"]),
            root: Some(words(&["cargo", "locate-project", "--workspace"])),
        });
        let ecosystems = vec![rust];
        let units = units(&[("libs/a", &[("rust", false)])]);
        let checks = plan_unit_checks(&mut context(&attempt), &folder(), &units, &ecosystems)
            .expect("the checks are planned");
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
        assert_eq!(
            reading.root.as_ref().map(line_of),
            Some(line(
                "root of lint_rust in libs/a",
                "/repository/libs/a",
                "cargo locate-project --workspace"
            )),
            "the root is asked in the folder the tool runs in"
        );
        let _ = fs::remove_dir_all(&attempt);
    }

    #[test]
    fn a_word_pointing_at_the_config_folder_is_given_where_the_configs_were_written() {
        let attempt = attempt("config");
        let ecosystems = vec![ecosystem(
            "python",
            &[(
                "lint_python",
                &["ruff", "check", "--config", "{pack}/ruff.toml"],
            )],
            &[("ruff.toml", "line-length = 100\n")],
        )];
        let units = units(&[("", &[("python", false)])]);
        let checks = plan_unit_checks(&mut context(&attempt), &folder(), &units, &ecosystems)
            .expect("the checks are planned");
        let written = attempt.join("config").join("python").join("ruff.toml");
        assert_eq!(
            checks[0].subprocess.arguments,
            ["check", "--config", &written.to_string_lossy()]
        );
        assert_eq!(
            fs::read_to_string(&written).expect("the config is written"),
            "line-length = 100\n"
        );
        let _ = fs::remove_dir_all(&attempt);
    }

    #[test]
    fn an_ecosystem_with_no_unit_plans_nothing_and_writes_no_config() {
        let attempt = attempt("none");
        let ecosystems = vec![ecosystem(
            "python",
            &[(
                "lint_python",
                &["ruff", "check", "--config", "{pack}/ruff.toml"],
            )],
            &[("ruff.toml", "line-length = 100\n")],
        )];
        let units = units(&[("", &[("rust", false)]), ("libs", &[("python", true)])]);
        let checks = plan_unit_checks(&mut context(&attempt), &folder(), &units, &ecosystems)
            .expect("nothing to check is no failure");
        assert!(checks.is_empty());
        assert!(!attempt.join("config").exists());
        let _ = fs::remove_dir_all(&attempt);
    }

    #[test]
    fn a_config_or_an_ecosystem_named_as_a_path_is_refused() {
        let attempt = attempt("escape");
        let units = units(&[("", &[("python", false)]), ("b", &[("../out", false)])]);
        let config = vec![ecosystem(
            "python",
            &[("lint_python", &["ruff"])],
            &[("../escape.toml", "x")],
        )];
        let failed = plan_unit_checks(&mut context(&attempt), &folder(), &units, &config)
            .expect_err("a config is written nowhere but in its own folder");
        assert_eq!(
            failed.reason,
            "the config ../escape.toml of python is not a plain file name"
        );
        let named = vec![ecosystem(
            "../out",
            &[("lint", &["ruff"])],
            &[("a.toml", "x")],
        )];
        let failed = plan_unit_checks(&mut context(&attempt), &folder(), &units, &named)
            .expect_err("an ecosystem's configs are written nowhere but under its own name");
        assert_eq!(
            failed.reason,
            "the ecosystem name ../out is not a plain file name"
        );
        assert!(!attempt.join("out").exists());
        let _ = fs::remove_dir_all(&attempt);
    }

    #[test]
    fn a_tool_or_a_root_naming_no_program_fails_even_with_no_unit_to_run_it_in() {
        let attempt = attempt("wordless");
        let ecosystems = vec![ecosystem("rust", &[("format_rust", &[])], &[])];
        let failed = plan_unit_checks(&mut context(&attempt), &folder(), &units(&[]), &ecosystems)
            .expect_err("nothing to start");
        assert_eq!(failed.reason, "the tool format_rust names no program");
        let mut rootless = ecosystem("rust", &[("lint_rust", &["cargo"])], &[]);
        rootless.tools[0].findings = Some(ReadableRun {
            reader: Reader::CargoJson,
            run: words(&["cargo", "clippy"]),
            root: Some(Vec::new()),
        });
        let failed = plan_unit_checks(&mut context(&attempt), &folder(), &units(&[]), &[rootless])
            .expect_err("no root to start");
        assert_eq!(
            failed.reason,
            "the root of the tool lint_rust names no program"
        );
        let _ = fs::remove_dir_all(&attempt);
    }
}
