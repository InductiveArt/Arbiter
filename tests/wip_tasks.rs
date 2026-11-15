use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use arbiter::commands::languages;
use arbiter::commands::tasks;
use arbiter::packs::all;
use arbiter::wip::job::id::RunId;
use arbiter::wip::plan::Plan;
use arbiter::wip::pull::Done;
use arbiter::wip::pull::Failed;
use arbiter::wip::pull::In;
use arbiter::wip::registry::known;
use arbiter::wip::run::report::Report;
use arbiter::wip::run::runner::Runner;
use arbiter::wip::scope::Scope;
use arbiter::wip::store::Store;
use arbiter::wip::task::Task;
use arbiter::wip::trace::outcome::Failure;
use arbiter::wip::trace::outcome::Outcome;
use arbiter::wip_domain::file::File;
use arbiter::wip_domain::finding::Finding;
use arbiter::wip_domain::folder::Folder;
use arbiter::wip_domain::unit::Unit;
use arbiter::wip_tasks::listed;
use arbiter::wip_tasks::load_ecosystems::load_ecosystems;

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_arbiter"))
}

fn folder(name: &str) -> PathBuf {
    let dir = env::temp_dir().join(format!(
        "arbiter-tasks-{name}-{}-{}",
        std::process::id(),
        RunId::next()
    ));
    fs::create_dir_all(&dir).expect("a folder is made");
    dir
}

#[test]
fn the_ecosystems_are_loaded_and_the_units_of_a_folder_found_each_task_in_a_process_of_its_own() {
    let repository = folder("repository");
    for (path, content) in [
        ("Cargo.toml", "[package]\n"),
        ("src/lib.rs", "pub fn one() {}\n"),
        (".gitignore", "/target\n"),
        ("target/debug/built", "x"),
        ("third_party/dep/lib.rs", "x"),
        ("tools/probe/pyproject.toml", "[project]\n"),
        ("tools/probe/probe.py", "x = 1\n"),
        ("libs/Cargo.toml", "[workspace]\nmembers = [\"a\"]\n"),
        ("libs/a/Cargo.toml", "[package]\n"),
        ("libs/a/src/lib.rs", "pub fn a() {}\n"),
        ("apps/other/.arbiter/units.toml", ""),
        ("apps/other/pyproject.toml", "[project]\n"),
        ("apps/other/main.py", "x = 1\n"),
    ] {
        let file = repository.join(path);
        fs::create_dir_all(file.parent().expect("a file sits in a folder"))
            .expect("a folder is made");
        fs::write(file, content).expect("a file is written");
    }
    let mut seeds = Store::new();
    seeds
        .put(&Folder {
            path: repository.clone(),
        })
        .expect("a folder encodes");
    let plan = Plan::new("units", &listed(), &["load_ecosystems", "find_units"])
        .expect("the two tasks wire into a plan");
    let runs = folder("runs");

    let report = Runner::new()
        .worker(binary())
        .run(&plan, seeds, &runs)
        .expect("the plan is run");

    assert_eq!(report.outcome, Outcome::Ok);
    let loading = report
        .step("load_ecosystems")
        .expect("the attempt left its record");
    assert_eq!(loading.measures.get("ecosystems"), Some(&4.0));
    assert_eq!(loading.measures.get("tools"), Some(&12.0));
    let units = report.get::<Vec<Unit>>().expect("the units are kept");
    let told: Vec<(String, String, String)> = units
        .iter()
        .map(|unit| {
            let markers: Vec<String> = unit
                .markers
                .iter()
                .map(|marker| match marker.members_only {
                    true => format!("{} listing members", marker.ecosystem),
                    false => marker.ecosystem.clone(),
                })
                .collect();
            let files: Vec<String> = unit
                .files
                .iter()
                .map(|file| file.path.to_string_lossy().into_owned())
                .collect();
            (
                unit.path.to_string_lossy().into_owned(),
                markers.join(", "),
                files.join(" "),
            )
        })
        .collect();
    let told: Vec<(&str, &str, &str)> = told
        .iter()
        .map(|(path, markers, files)| (path.as_str(), markers.as_str(), files.as_str()))
        .collect();
    assert_eq!(
        told,
        [
            ("", "rust", ".gitignore Cargo.toml src/lib.rs"),
            ("libs", "rust listing members", "libs/Cargo.toml"),
            ("libs/a", "rust", "libs/a/Cargo.toml libs/a/src/lib.rs"),
            (
                "tools/probe",
                "python",
                "tools/probe/probe.py tools/probe/pyproject.toml"
            ),
        ]
    );
    let found = report
        .step("find_units")
        .expect("the attempt left its record");
    let steps: Vec<&str> = found
        .children
        .iter()
        .map(|step| step.name.as_str())
        .collect();
    assert_eq!(steps, ["list_files", "read_markers", "group_files_by_unit"]);
    let listing = &found.children[0];
    assert_eq!(listing.measures.get("files"), Some(&8.0));
    assert_eq!(listing.measures.get("ignored"), Some(&1.0));
    assert_eq!(listing.measures.get("skipped"), Some(&1.0));
    assert_eq!(listing.measures.get("foreign"), Some(&1.0));
    assert_eq!(listing.evidence, ["foreign: apps/other"]);
    let reading = &found.children[1];
    assert_eq!(reading.measures.get("markers"), Some(&4.0));
    assert_eq!(reading.measures.get("members_only"), Some(&1.0));
    let grouping = &found.children[2];
    assert_eq!(grouping.measures.get("units"), Some(&4.0));
    let _ = fs::remove_dir_all(&repository);
    let _ = fs::remove_dir_all(&runs);
}

#[test]
fn no_task_of_the_gate_bears_a_name_the_exhibit_already_does() {
    let mut seen = HashSet::new();
    for task in listed().into_iter().chain(known()) {
        assert!(
            seen.insert(task.name.clone()),
            "two tasks are named {}: a worker finds a body by its name",
            task.name
        );
    }
}

#[test]
fn the_tasks_view_tells_what_each_task_leaves_reads_and_is_bounded_by() {
    assert_eq!(
        tasks::shown(&listed()),
        "load_ecosystems\n  \
         every ecosystem this binary was built with, as its record declares it: its unit \
         marker, its members-only rule, its tools and the config files they are pointed at\n  \
         reads   nothing\n  \
         leaves  Vec<Ecosystem>\n  \
         limits  no time limit, one attempt, computed every run\n\
         \n\
         find_units\n  \
         each unit under one folder with its markers and its own files: the folder itself and \
         any folder under it holding an ecosystem's marker; what a .gitignore leaves out, \
         third_party, dot-folders, links and any folder below holding its own .arbiter are \
         never read\n  \
         reads   Folder, Vec<Ecosystem>\n  \
         leaves  Vec<Unit>\n  \
         limits  no time limit, one attempt, computed every run\n\
         \n\
         run_ecosystem_tools\n  \
         each tool of each ecosystem once in every unit of that ecosystem, except a unit whose \
         marker only lists members, run one after the other, each as a run_subprocess task; \
         what a tool prints is read into findings when it has a reader, and a run that does \
         not exit with 0 and leaves no finding is one finding\n  \
         reads   Folder, Vec<Unit>, Vec<Ecosystem>\n  \
         leaves  Vec<Finding>\n  \
         limits  no time limit, one attempt, computed every run\n\
         \n\
         run_subprocess\n  \
         starts one program in its folder, keeps all it prints and tells how it ended; an \
         exit other than 0 is told, never failed\n  \
         reads   Subprocess\n  \
         leaves  Exit\n  \
         limits  1800s at most, one attempt, computed every run\n"
    );
}

#[test]
fn the_tasks_view_tells_when_a_read_fires_and_every_limit_a_task_sets() {
    let shaped = Task::new(
        "mend",
        |_: &mut Scope, (_held, _broke, _ended): (In<Vec<File>>, Failed<Unit>, Done<Vec<Unit>>)| {
            Ok::<u8, Failure>(0)
        },
    )
    .within(Duration::from_secs(30))
    .retries(2)
    .cached();
    let alone = Task::new("seed", |_: &mut Scope, (): ()| {
        Ok::<Unit, Failure>(Unit {
            path: PathBuf::new(),
            markers: Vec::new(),
            files: Vec::new(),
        })
    });
    assert_eq!(
        tasks::shown(&[shaped, alone]),
        "mend\n  \
         reads   Vec<File>, Unit when its task failed, Vec<Unit> however its task ended\n  \
         leaves  u8\n  \
         limits  30s at most, up to 3 attempts, replayed for the same inputs\n\
         \n\
         seed\n  \
         reads   nothing\n  \
         leaves  Unit\n  \
         limits  no time limit, one attempt, computed every run\n"
    );
}

#[test]
fn the_languages_view_tells_each_language_s_marker_rule_checks_needs_locks_and_outputs() {
    let told = languages::shown(&all(), &load_ecosystems().expect("the records are read"));
    let rows: Vec<Vec<&str>> = told
        .lines()
        .map(|line| {
            line.split("  ")
                .map(str::trim)
                .filter(|cell| !cell.is_empty())
                .collect()
        })
        .collect();
    assert_eq!(
        rows,
        [
            vec![
                "language",
                "unit marker",
                "members only when",
                "checks",
                "needs",
                "lockfiles",
                "never read"
            ],
            vec!["cpp", "none", "never", "none", "none", "none", "none"],
            vec!["infra", "none", "never", "none", "none", "none", "none"],
            vec![
                "proto",
                "buf.yaml",
                "never",
                "format, lint",
                "npx",
                "none",
                "none"
            ],
            vec![
                "python",
                "pyproject.toml",
                "has tool.uv.workspace, lacks project",
                "format, lint, types, test",
                "uv",
                "uv.lock",
                "__pycache__"
            ],
            vec![
                "rust",
                "Cargo.toml",
                "has workspace, lacks package",
                "fmt, clippy, test",
                "cargo",
                "Cargo.lock",
                "target"
            ],
            vec![
                "typescript",
                "package.json",
                "has workspaces, lacks main, exports, bin",
                "lint, types, test",
                "node, npx",
                "pnpm-lock.yaml, package-lock.json",
                "node_modules, dist"
            ],
        ]
    );
}

#[test]
fn the_binary_prints_both_views() {
    for (view, first) in [("tasks", "load_ecosystems"), ("languages", "language")] {
        let ran = Command::new(binary())
            .arg(view)
            .output()
            .expect("the binary runs");
        assert!(ran.status.success(), "arbiter {view} exits clean");
        let printed = String::from_utf8_lossy(&ran.stdout);
        assert!(
            printed.starts_with(first),
            "arbiter {view} printed: {printed}"
        );
    }
}

fn repository(name: &str, written: &[(&str, &str)]) -> PathBuf {
    let root = folder(name);
    for (path, content) in written {
        let file = root.join(path);
        fs::create_dir_all(file.parent().expect("a file sits in a folder"))
            .expect("a folder is made");
        fs::write(file, content).expect("a file is written");
    }
    root
}

fn seeded(repository: &Path) -> Store {
    let mut seeds = Store::new();
    seeds
        .put(&Folder {
            path: repository.to_path_buf(),
        })
        .expect("a folder encodes");
    seeds
}

// Each run a task made, by its name, and whether it could be carried out.
fn ended(report: &Report, task: &str) -> Vec<(String, Outcome)> {
    report
        .step(task)
        .expect("the attempt left its record")
        .children
        .iter()
        .map(|run| (run.name.clone(), run.outcome.clone()))
        .collect()
}

// What one run printed, read from the file the record of its task names.
fn printed(report: &Report, task: &str, run: &str) -> String {
    let named = report
        .step(task)
        .expect("the attempt left its record")
        .children
        .iter()
        .find(|inner| inner.name == run)
        .and_then(|inner| {
            inner.children.iter().find_map(|attempt| {
                attempt
                    .evidence
                    .iter()
                    .find_map(|line| line.strip_prefix("stdout: "))
            })
        })
        .expect("the record names the file");
    fs::read_to_string(named).expect("the file is read")
}

// Each finding as its check, its path, its rule and its line.
fn found(findings: &[Finding]) -> Vec<(&str, &str, Option<&str>, Option<u32>)> {
    findings
        .iter()
        .map(|finding| {
            (
                finding.check.as_str(),
                finding.path.to_str().expect("a path in UTF-8"),
                finding.rule.as_deref(),
                finding.line,
            )
        })
        .collect()
}

#[test]
fn the_tools_an_ecosystem_declares_are_run_in_order_in_every_unit_each_as_a_task_of_its_own() {
    let repository = repository(
        "tools",
        &[
            ("Cargo.toml", "[package]\n"),
            ("src/lib.rs", "pub fn one() {}\n"),
            ("libs/a/Cargo.toml", "[package]\nname = \"a\"\n"),
            ("libs/a/src/lib.rs", "pub fn a() {}\n"),
        ],
    );
    let shared = folder("shared");
    let mark = shared.join("ended");
    let mut ecosystems = load_ecosystems().expect("the records are read");
    for tool in ecosystems
        .iter_mut()
        .flat_map(|ecosystem| ecosystem.tools.iter_mut())
    {
        let words = match tool.name.as_str() {
            "format_rust" => vec![
                "sh".to_string(),
                "-c".to_string(),
                format!("touch '{}'; exit 1", mark.display()),
            ],
            "lint_rust" => vec![
                "sh".to_string(),
                "-c".to_string(),
                format!("test -f '{}'", mark.display()),
            ],
            "test_rust" => vec!["cat".to_string(), "Cargo.toml".to_string()],
            _ => continue,
        };
        tool.run = words;
        tool.findings = None;
    }
    let mut seeds = seeded(&repository);
    seeds.put(&ecosystems).expect("the ecosystems encode");
    let plan = Plan::new("tools", &listed(), &["find_units", "run_ecosystem_tools"])
        .expect("the two tasks wire into a plan");
    let runs = folder("runs");

    let report = Runner::new()
        .worker(binary())
        .run(&plan, seeds, &runs)
        .expect("the plan is run");

    assert_eq!(
        ended(&report, "run_ecosystem_tools"),
        [
            ("format_rust in .".to_string(), Outcome::Ok),
            ("format_rust in libs/a".to_string(), Outcome::Ok),
            ("lint_rust in .".to_string(), Outcome::Ok),
            ("lint_rust in libs/a".to_string(), Outcome::Ok),
            ("test_rust in .".to_string(), Outcome::Ok),
            ("test_rust in libs/a".to_string(), Outcome::Ok),
        ],
        "every run is carried out, one after the other"
    );
    let tools = report
        .step("run_ecosystem_tools")
        .expect("the attempt left its record");
    assert!(
        tools.children.iter().all(|run| run.children.len() == 1
            && run.children[0].name == "run_subprocess"
            && run.children[0].account.is_some()),
        "each run was a run_subprocess task in a process of its own"
    );
    let findings = report.get::<Vec<Finding>>().expect("the findings are kept");
    assert_eq!(
        found(&findings),
        [
            ("format_rust", "", None, None),
            ("format_rust", "libs/a", None, None)
        ],
        "a run with no reader that does not exit with 0 is one finding on its unit, and the \
         lint found its mark because the format ran before it"
    );
    assert!(
        findings.iter().all(|finding| finding
            .message
            .starts_with("sh exited with 1; all it printed is kept in ")),
        "{findings:?}"
    );
    assert_eq!(
        printed(&report, "run_ecosystem_tools", "test_rust in libs/a"),
        "[package]\nname = \"a\"\n"
    );
    let _ = fs::remove_dir_all(&repository);
    let _ = fs::remove_dir_all(&shared);
    let _ = fs::remove_dir_all(&runs);
}

#[test]
fn the_rust_tools_as_the_built_in_record_writes_them_place_each_fault_in_its_file_in_a_crate_or_a_workspace(
) {
    let manifest = |name: &str| {
        format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n")
    };
    // A crate that is its own workspace, so the one at the top does not claim it.
    let alone = |name: &str| format!("{}[workspace]\n", manifest(name));
    let repository = repository(
        "crates",
        &[
            (
                "Cargo.toml",
                "[workspace]\nmembers = [\"libs/a\"]\nresolver = \"2\"\n",
            ),
            ("libs/a/Cargo.toml", &manifest("a")),
            (
                "libs/a/src/lib.rs",
                "pub fn same(x: u32) -> bool {\n    x == x\n}\n",
            ),
            ("clean/Cargo.toml", &alone("clean")),
            ("clean/src/lib.rs", "pub fn one() {}\n"),
            ("solo/Cargo.toml", &alone("solo")),
            (
                "solo/src/lib.rs",
                "pub fn same(x: u32) -> bool{\n    x == x\n}\n",
            ),
        ],
    );
    let plan = Plan::new(
        "gate",
        &listed(),
        &["load_ecosystems", "find_units", "run_ecosystem_tools"],
    )
    .expect("the three tasks wire into a plan");
    let runs = folder("runs");

    let report = Runner::new()
        .worker(binary())
        .run(&plan, seeded(&repository), &runs)
        .expect("the plan is run");

    let carried = ended(&report, "run_ecosystem_tools");
    assert_eq!(carried.len(), 9, "{carried:?}");
    assert!(
        carried.iter().all(|(_, outcome)| *outcome == Outcome::Ok),
        "{carried:?}"
    );
    let findings = report.get::<Vec<Finding>>().expect("the findings are kept");
    assert_eq!(
        found(&findings),
        [
            ("format_rust", "solo", None, None),
            (
                "lint_rust",
                "libs/a/src/lib.rs",
                Some("clippy::eq_op"),
                Some(2)
            ),
            (
                "lint_rust",
                "solo/src/lib.rs",
                Some("clippy::eq_op"),
                Some(2)
            ),
        ],
        "a member's file is placed from the root of the workspace cargo wrote it from"
    );
    assert!(
        findings[0].message.starts_with("cargo exited with 1; "),
        "{}",
        findings[0].message
    );
    let told = printed(&report, "run_ecosystem_tools", "format_rust in solo");
    assert!(
        told.contains("bool {"),
        "the tool's own output is kept: {told}"
    );
    let _ = fs::remove_dir_all(&repository);
    let _ = fs::remove_dir_all(&runs);
}
