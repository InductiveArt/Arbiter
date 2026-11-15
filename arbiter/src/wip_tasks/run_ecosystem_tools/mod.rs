mod read_cargo_json;

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use task_runner::plan::Plan;
use task_runner::pull::In;
use task_runner::run::runner::run_within;
use task_runner::scope::Scope;
use task_runner::store::Store;
use task_runner::task::Task;
use task_runner::trace::outcome::Failure;
use task_runner::trace::outcome::Outcome;

use crate::wip_domain::diagnostic::Diagnostic;
use crate::wip_domain::ecosystem::Ecosystem;
use crate::wip_domain::ecosystem::Reader;
use crate::wip_domain::ecosystem::Tool;
use crate::wip_domain::finding::Finding;
use crate::wip_domain::folder::Folder;
use crate::wip_domain::subprocess::Exit;
use crate::wip_domain::subprocess::Subprocess;
use crate::wip_domain::unit::Unit;
use crate::wip_tasks::run_ecosystem_tools::read_cargo_json::read_cargo_json;
use crate::wip_tasks::run_ecosystem_tools::read_cargo_json::workspace_root;
use crate::wip_tasks::run_subprocess::run_subprocess_task;

pub fn run_ecosystem_tools_task() -> Task {
    Task::new(
        "run_ecosystem_tools",
        |scope: &mut Scope,
         (folder, units, ecosystems): (In<Folder>, In<Vec<Unit>>, In<Vec<Ecosystem>>)| {
            run_ecosystem_tools(scope, &folder, &units, &ecosystems)
        },
    )
    .line(
        "each tool of each ecosystem once in every unit of that ecosystem, except a unit whose \
         marker only lists members, run one after the other, each as a run_subprocess task; \
         what a tool prints is read into findings when it has a reader, and a run that does \
         not exit with 0 and leaves no finding is one finding",
    )
}

// Every run is tried even after one could not be run, so one attempt tells
// each that could not.
fn run_ecosystem_tools(
    scope: &mut Scope,
    folder: &Folder,
    units: &[Unit],
    ecosystems: &[Ecosystem],
) -> Result<Vec<Finding>, Failure> {
    let runs = planned(scope, folder, units, ecosystems)?;
    let mut findings = Vec::new();
    let mut failed: Vec<&str> = Vec::new();
    for tool_run in &runs {
        let name = &tool_run.subprocess.name;
        match scope.step(name, |step| run_tool(step, folder, tool_run)) {
            Ok(found) => findings.extend(found),
            Err(_) => {
                // A job told to stop ends here; any other failure is the run's own.
                scope.checkpoint()?;
                failed.push(name);
            }
        }
    }
    scope.measure("runs", runs.len() as f64);
    scope.measure("failed", failed.len() as f64);
    scope.measure("findings", findings.len() as f64);
    if failed.is_empty() {
        Ok(findings)
    } else {
        Err(Failure::from(format!(
            "{} of {} could not be run: {}",
            failed.len(),
            runs.len(),
            failed.join(", ")
        )))
    }
}

// One tool to run in one unit: the program starting it, and the program
// printing the root its paths run from when its reading names one.
#[derive(Debug)]
struct ToolRun<'a> {
    tool: &'a Tool,
    unit: &'a Unit,
    subprocess: Subprocess,
    root: Option<Subprocess>,
}

// Decides what is run, in order, and starts nothing: ecosystems in the order
// given, the tools of each in the order its record writes them, each tool once
// per unit.
fn planned<'a>(
    scope: &mut Scope,
    folder: &Folder,
    units: &'a [Unit],
    ecosystems: &'a [Ecosystem],
) -> Result<Vec<ToolRun<'a>>, Failure> {
    let mut runs = Vec::new();
    for ecosystem in ecosystems {
        let held: Vec<&Unit> = units
            .iter()
            .filter(|unit| {
                unit.markers
                    .iter()
                    .any(|marker| marker.ecosystem == ecosystem.name && !marker.members_only)
            })
            .collect();
        let configs = scope.dir().join("config").join(&ecosystem.name);
        let pointed = configs.to_string_lossy();
        for tool in &ecosystem.tools {
            // A tool whose findings are read is run by the words that print them.
            let run = tool
                .findings
                .as_ref()
                .map_or(&tool.run, |readable| &readable.run);
            let Some((program, arguments)) = split(run, &pointed) else {
                return Err(Failure::from(format!(
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
                    Failure::from(format!(
                        "the root of the tool {} names no program",
                        tool.name
                    ))
                })?),
            };
            for unit in &held {
                let at = folder.path.join(&unit.path);
                let located = root.as_ref().map(|(program, arguments)| Subprocess {
                    name: "root".to_string(),
                    program: program.clone(),
                    arguments: arguments.clone(),
                    folder: at.clone(),
                });
                let subprocess = Subprocess {
                    name: format!("{} in {}", tool.name, shown(unit)),
                    program: program.clone(),
                    arguments: arguments.clone(),
                    folder: at,
                };
                runs.push(ToolRun {
                    tool,
                    unit,
                    subprocess,
                    root: located,
                });
            }
        }
        if !held.is_empty() {
            write_configs(&configs, ecosystem)?;
        }
    }
    Ok(runs)
}

// The words with `{pack}` given the config folder, as the program and the
// arguments handed to it.
fn split(words: &[String], pointed: &str) -> Option<(String, Vec<String>)> {
    let mut given = words.iter().map(|word| word.replace("{pack}", pointed));
    let program = given.next()?;
    Some((program, given.collect()))
}

// One run, then what it printed read into findings. The root is asked only
// when a finding needs placing: a tool that could not read its own project
// prints nothing to place, and asking its root would fail the same way.
fn run_tool(
    scope: &mut Scope,
    folder: &Folder,
    tool_run: &ToolRun,
) -> Result<Vec<Finding>, Failure> {
    let ToolRun {
        tool,
        unit,
        subprocess,
        root,
    } = tool_run;
    let exit = run(scope, subprocess)?;
    let mut found = match &tool.findings {
        None => Vec::new(),
        Some(readable) => {
            let printed = stdout(&exit)?;
            let diagnostics = match readable.reader {
                Reader::CargoJson => read_cargo_json(&printed),
            };
            if diagnostics.is_empty() {
                Vec::new()
            } else {
                let base = match root {
                    None => subprocess.folder.clone(),
                    Some(root) => {
                        let located = scope.step(&root.name, |step| run(step, root))?;
                        if located.code != Some(0) {
                            return Err(Failure::from(format!(
                                "no root for its paths: {}",
                                located.ended(&root.program)
                            )));
                        }
                        let told = stdout(&located)?;
                        match readable.reader {
                            Reader::CargoJson => workspace_root(&told)?,
                        }
                    }
                };
                placed(tool, diagnostics, &base, &folder.path)?
            }
        }
    };
    if exit.code != Some(0) && found.is_empty() {
        found.push(Finding {
            check: tool.name.clone(),
            rule: None,
            path: unit.path.clone(),
            line: None,
            message: format!(
                "{}; all it printed is kept in {} and {}",
                exit.ended(&subprocess.program),
                exit.stdout.display(),
                exit.stderr.display()
            ),
        });
    }
    scope.measure("findings", found.len() as f64);
    Ok(found)
}

// One program as a run_subprocess task of its own, inside this attempt.
fn run(scope: &mut Scope, subprocess: &Subprocess) -> Result<Exit, Failure> {
    let plan = Plan::new(
        "run_subprocess",
        &[run_subprocess_task()],
        &["run_subprocess"],
    )
    .map_err(|invalid| Failure::from(format!("{invalid:?}")))?;
    let mut seeds = Store::new();
    seeds
        .put(subprocess)
        .map_err(|error| Failure::from(error.to_string()))?;
    let report =
        run_within(scope, &plan, seeds).map_err(|refused| Failure::from(refused.to_string()))?;
    let outcome = report
        .step("run_subprocess")
        .map(|step| step.outcome.clone())
        .ok_or_else(|| Failure::from("the run left no record"))?;
    match outcome {
        Outcome::Ok => report
            .get::<Exit>()
            .ok_or_else(|| Failure::from("the run left no exit")),
        Outcome::Failed(reason) => Err(Failure::from(reason)),
        Outcome::TimedOut => Err(Failure::from("the run timed out")),
        Outcome::Cancelled => Err(Failure::from("the run was cancelled")),
        other => Err(Failure::from(format!("the run ended {other:?}"))),
    }
}

// All the program printed on its standard output.
fn stdout(exit: &Exit) -> Result<String, Failure> {
    let printed = fs::read(&exit.stdout).map_err(|error| {
        Failure::from(format!(
            "{} could not be read: {error}",
            exit.stdout.display()
        ))
    })?;
    Ok(String::from_utf8_lossy(&printed).into_owned())
}

// Each diagnostic as a finding of the tool, its file by path from the listed
// folder when it is under it and by its whole path when it is not. Both
// folders are taken as the disk resolves them, so a link in the way of either
// does not move a file out of its place.
fn placed(
    tool: &Tool,
    diagnostics: Vec<Diagnostic>,
    base: &Path,
    listed: &Path,
) -> Result<Vec<Finding>, Failure> {
    let base = resolved(base)?;
    let listed = resolved(listed)?;
    Ok(diagnostics
        .into_iter()
        .map(|diagnostic| {
            let whole = base.join(&diagnostic.file);
            Finding {
                check: tool.name.clone(),
                rule: diagnostic.code,
                path: whole
                    .strip_prefix(&listed)
                    .map_or_else(|_| whole.clone(), Path::to_path_buf),
                line: diagnostic.line,
                message: diagnostic.message,
            }
        })
        .collect())
}

fn resolved(folder: &Path) -> Result<PathBuf, Failure> {
    fs::canonicalize(folder).map_err(|error| {
        Failure::from(format!(
            "{} could not be resolved: {error}",
            folder.display()
        ))
    })
}

// The ecosystem's config files, written inside the attempt's own folder so
// nothing changes in the folder the tools are run on.
fn write_configs(into: &Path, ecosystem: &Ecosystem) -> Result<(), Failure> {
    if ecosystem.configs.is_empty() {
        return Ok(());
    }
    if !plain(&ecosystem.name) {
        return Err(Failure::from(format!(
            "the ecosystem name {} is not a plain file name",
            ecosystem.name
        )));
    }
    fs::create_dir_all(into).map_err(|error| unwritten(into, &error))?;
    for config in &ecosystem.configs {
        if !plain(&config.name) {
            return Err(Failure::from(format!(
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

fn unwritten(path: &Path, error: &io::Error) -> Failure {
    Failure::from(format!("{} could not be written: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;

    use task_runner::job::cancel::Cancel;
    use task_runner::job::id::JobId;
    use task_runner::job::id::RunId;
    use task_runner::layout::Site;
    use task_runner::scope::Scope;
    use task_runner::trace::outcome::Outcome;

    use super::placed;
    use super::planned;
    use super::ToolRun;
    use crate::wip_domain::diagnostic::Diagnostic;
    use crate::wip_domain::ecosystem::Config;
    use crate::wip_domain::ecosystem::Ecosystem;
    use crate::wip_domain::ecosystem::ReadableRun;
    use crate::wip_domain::ecosystem::Reader;
    use crate::wip_domain::ecosystem::Tool;
    use crate::wip_domain::folder::Folder;
    use crate::wip_domain::marker::Marker;
    use crate::wip_domain::subprocess::Subprocess;
    use crate::wip_domain::unit::Unit;

    // The attempt's own folder, one per test.
    fn attempt(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!(
            "arbiter-tools-{name}-{}-{}",
            std::process::id(),
            RunId::next()
        ));
        fs::create_dir_all(&dir).expect("the folder is made");
        dir
    }

    fn scope(attempt: &Path) -> Scope {
        let site = Site {
            dir: attempt.to_path_buf(),
            store: attempt.join("store"),
        };
        Scope::new(
            JobId::new(RunId::next(), "run_ecosystem_tools", 1),
            Cancel::new(),
            site,
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

    // Each program by the name of its run, its folder and its whole line.
    fn told(runs: &[ToolRun]) -> Vec<(String, String, String)> {
        runs.iter()
            .map(|tool_run| line_of(&tool_run.subprocess))
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
    fn each_tool_is_run_once_per_unit_of_its_ecosystem_in_the_order_written() {
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
        let mut scope = scope(&attempt);
        let runs =
            planned(&mut scope, &folder(), &units, &ecosystems).expect("the runs are decided");
        assert_eq!(
            told(&runs),
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
        assert!(
            runs.iter().all(|tool_run| tool_run.root.is_none()),
            "a tool whose findings are not read asks no root"
        );
        let step = scope.finished("run_ecosystem_tools", Outcome::Ok);
        assert!(
            step.resources.children.is_empty(),
            "deciding starts no program"
        );
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
        let runs = planned(&mut scope(&attempt), &folder(), &units, &ecosystems)
            .expect("the runs are decided");
        assert_eq!(
            told(&runs),
            [line(
                "lint_rust in libs/a",
                "/repository/libs/a",
                "cargo clippy --message-format=json"
            )]
        );
        assert_eq!(
            runs[0].root.as_ref().map(line_of),
            Some(line(
                "root",
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
        let runs = planned(&mut scope(&attempt), &folder(), &units, &ecosystems)
            .expect("the runs are decided");
        let written = attempt.join("config").join("python").join("ruff.toml");
        assert_eq!(
            runs[0].subprocess.arguments,
            ["check", "--config", &written.to_string_lossy()]
        );
        assert_eq!(
            fs::read_to_string(&written).expect("the config is written"),
            "line-length = 100\n"
        );
        let _ = fs::remove_dir_all(&attempt);
    }

    #[test]
    fn an_ecosystem_with_no_unit_runs_nothing_and_writes_no_config() {
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
        let runs = planned(&mut scope(&attempt), &folder(), &units, &ecosystems)
            .expect("nothing to run is no failure");
        assert!(runs.is_empty());
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
        let failed = planned(&mut scope(&attempt), &folder(), &units, &config)
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
        let failed = planned(&mut scope(&attempt), &folder(), &units, &named)
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
        let failed = planned(&mut scope(&attempt), &folder(), &units(&[]), &ecosystems)
            .expect_err("nothing to start");
        assert_eq!(failed.reason, "the tool format_rust names no program");
        let mut rootless = ecosystem("rust", &[("lint_rust", &["cargo"])], &[]);
        rootless.tools[0].findings = Some(ReadableRun {
            reader: Reader::CargoJson,
            run: words(&["cargo", "clippy"]),
            root: Some(Vec::new()),
        });
        let failed = planned(&mut scope(&attempt), &folder(), &units(&[]), &[rootless])
            .expect_err("no root to start");
        assert_eq!(
            failed.reason,
            "the root of the tool lint_rust names no program"
        );
        let _ = fs::remove_dir_all(&attempt);
    }

    #[test]
    fn a_file_under_the_listed_folder_is_told_from_it_even_through_a_link_and_one_outside_whole() {
        let attempt = attempt("placed");
        let member = attempt.join("libs").join("a");
        fs::create_dir_all(&member).expect("the folder is made");
        // The base as the disk resolves it, the listed folder as written: on a
        // system whose temporary folder sits behind a link the two differ.
        let base = fs::canonicalize(&attempt).expect("the folder resolves");
        let tool = Tool {
            name: "lint_rust".to_string(),
            run: Vec::new(),
            findings: None,
        };
        let diagnostic = |file: &str| Diagnostic {
            file: PathBuf::from(file),
            line: Some(2),
            code: Some("clippy::eq_op".to_string()),
            message: "told".to_string(),
        };
        let found = placed(
            &tool,
            vec![
                diagnostic("libs/a/src/lib.rs"),
                diagnostic("/elsewhere/x.rs"),
            ],
            &base,
            &attempt,
        )
        .expect("both folders resolve");
        let paths: Vec<&Path> = found.iter().map(|finding| finding.path.as_path()).collect();
        assert_eq!(
            paths,
            [Path::new("libs/a/src/lib.rs"), Path::new("/elsewhere/x.rs")]
        );
        assert_eq!(found[0].check, "lint_rust");
        assert_eq!(found[0].rule.as_deref(), Some("clippy::eq_op"));
        let _ = fs::remove_dir_all(&attempt);
    }
}
