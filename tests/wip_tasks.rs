use std::collections::HashSet;
use std::env;
use std::fs;
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
use arbiter::wip::run::runner::Runner;
use arbiter::wip::scope::Scope;
use arbiter::wip::store::Store;
use arbiter::wip::task::Task;
use arbiter::wip::trace::outcome::Failure;
use arbiter::wip::trace::outcome::Outcome;
use arbiter::wip_tasks::group_files_by_unit::Markers;
use arbiter::wip_tasks::group_files_by_unit::Unit;
use arbiter::wip_tasks::group_files_by_unit::Units;
use arbiter::wip_tasks::list_files::Files;
use arbiter::wip_tasks::list_files::Folder;
use arbiter::wip_tasks::listed;

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
fn a_folder_is_listed_then_grouped_as_one_plan_each_task_in_a_process_of_its_own() {
    let repository = folder("repository");
    for (path, content) in [
        ("Cargo.toml", "[package]\n"),
        ("src/lib.rs", "pub fn one() {}\n"),
        (".gitignore", "/target\n"),
        ("target/debug/built", "x"),
        ("third_party/dep/lib.rs", "x"),
        ("tools/probe/pyproject.toml", "[project]\n"),
        ("tools/probe/probe.py", "x = 1\n"),
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
    seeds
        .put(&Markers {
            names: vec!["Cargo.toml".to_string(), "pyproject.toml".to_string()],
        })
        .expect("markers encode");
    let plan = Plan::new("units", &listed(), &["list_files", "group_files_by_unit"])
        .expect("the two tasks wire into a plan");
    let runs = folder("runs");

    let report = Runner::new()
        .worker(binary())
        .run(&plan, seeds, &runs)
        .expect("the plan is run");

    assert_eq!(report.outcome, Outcome::Ok);
    let files = report.get::<Files>().expect("the list is kept");
    let paths: Vec<String> = files
        .files
        .iter()
        .map(|file| file.path.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        paths,
        [
            ".gitignore",
            "Cargo.toml",
            "src/lib.rs",
            "tools/probe/probe.py",
            "tools/probe/pyproject.toml"
        ]
    );
    let units = report.get::<Units>().expect("the units are kept");
    let told: Vec<(String, Vec<String>, usize)> = units
        .units
        .iter()
        .map(|unit| {
            (
                unit.path.to_string_lossy().into_owned(),
                unit.markers.clone(),
                unit.files.len(),
            )
        })
        .collect();
    assert_eq!(
        told,
        [
            (String::new(), vec!["Cargo.toml".to_string()], 3),
            (
                "tools/probe".to_string(),
                vec!["pyproject.toml".to_string()],
                2
            ),
        ]
    );
    let listing = report
        .step("list_files")
        .expect("the attempt left its record");
    assert_eq!(listing.measures.get("files"), Some(&5.0));
    assert_eq!(listing.measures.get("ignored"), Some(&1.0));
    assert_eq!(listing.measures.get("skipped"), Some(&1.0));
    assert_eq!(listing.measures.get("foreign"), Some(&1.0));
    assert_eq!(listing.evidence, ["foreign: apps/other"]);
    let grouping = report
        .step("group_files_by_unit")
        .expect("the attempt left its record");
    assert_eq!(grouping.measures.get("units"), Some(&2.0));
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
        "list_files\n  \
         every file under one folder with its size and the digest of its content, less what a \
         .gitignore leaves out; third_party, dot-folders, links and any folder below it holding \
         its own .arbiter are never read\n  \
         reads   Folder\n  \
         leaves  Files\n  \
         limits  no time limit, one attempt, computed every run\n\
         \n\
         group_files_by_unit\n  \
         each unit with its own files: a unit is the listed folder or any folder under it \
         holding a marker, and a file goes to the deepest unit above it\n  \
         reads   Files, Markers\n  \
         leaves  Units\n  \
         limits  no time limit, one attempt, computed every run\n"
    );
}

#[test]
fn the_tasks_view_tells_when_a_read_fires_and_every_limit_a_task_sets() {
    let shaped = Task::new(
        "mend",
        |_: &mut Scope, (_held, _broke, _ended): (In<Files>, Failed<Unit>, Done<Vec<Files>>)| {
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
         reads   Files, Unit when its task failed, Vec<Files> however its task ended\n  \
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
fn the_languages_view_tells_each_language_s_marker_checks_needs_locks_and_outputs() {
    let told = languages::shown(&all());
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
                "checks",
                "needs",
                "lockfiles",
                "never read"
            ],
            vec!["cpp", "none", "none", "none", "none", "none"],
            vec!["infra", "none", "none", "none", "none", "none"],
            vec!["proto", "buf.yaml", "format, lint", "npx", "none", "none"],
            vec![
                "python",
                "pyproject.toml",
                "format, lint, types, test",
                "uv",
                "uv.lock",
                "__pycache__"
            ],
            vec![
                "rust",
                "Cargo.toml",
                "fmt, clippy, test",
                "cargo",
                "Cargo.lock",
                "target"
            ],
            vec![
                "typescript",
                "package.json",
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
    for (view, first) in [("tasks", "list_files"), ("languages", "language")] {
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
