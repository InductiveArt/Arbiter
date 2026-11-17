use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use arbiter::commands::languages;
use arbiter::commands::tasks;
use arbiter::domain::dials::Dials;
use arbiter::domain::file::File;
use arbiter::domain::finding::Finding;
use arbiter::domain::folder::Folder;
use arbiter::domain::found_units::FoundUnits;
use arbiter::domain::gate_mode::GateMode;
use arbiter::domain::generated_checks::GeneratedChecks;
use arbiter::domain::law::LawFindings;
use arbiter::domain::law::LawName;
use arbiter::domain::repairs::Repairs;
use arbiter::domain::subprocess::Subprocess;
use arbiter::domain::subprocess::SubprocessExit;
use arbiter::domain::unit::Unit;
use arbiter::gate::listed;
use arbiter::gate::load_ecosystems::load_ecosystems;
use arbiter::packs::all;
use arbiter::packs::graphed;
use arbiter::report::files;
use arbiter::report::of_plan;
use arbiter::tasks::laws;

use task_runner::context::task_context::TaskContext;
use task_runner::contract::RunId;
use task_runner::contract::RunRecord;
use task_runner::contract::RunReport;
use task_runner::declaration::fan_out::FanOut;
use task_runner::declaration::plan::Plan;
use task_runner::declaration::pull::Done;
use task_runner::declaration::pull::Failed;
use task_runner::declaration::pull::In;
use task_runner::declaration::task::Task;
use task_runner::execution::runner::Runner;
use task_runner::record::task_failure::TaskFailure;
use task_runner::value::run_store::RunStore;

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_arbiter"))
}

// The runner every plan here is run by: the arbiter binary performs each
// attempt, as the gate itself has it do.
fn runner() -> Runner {
    Runner::new()
        .task_executable(binary())
        .task_arguments(&["attempt"])
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
    let mut seeds = RunStore::new();
    seeds
        .put(&Folder {
            path: repository.clone(),
        })
        .expect("a folder encodes");
    let plan = Plan::new("units", &listed(), &["load_ecosystems", "find_units"])
        .expect("the two tasks wire into a plan");
    let runs = folder("runs");

    let (report, store) = runner().run(&plan, seeds, &runs).expect("the plan is run");

    assert_eq!(*report.outcome(), RunRecord::passed());
    let loading = report
        .attempt("load_ecosystems")
        .expect("the attempt left its record");
    assert_eq!(loading.measures.get("ecosystems"), Some(&4.0));
    assert_eq!(loading.measures.get("tools"), Some(&12.0));
    let found = store.get::<FoundUnits>().expect("the units are kept");
    let told: Vec<(String, String, String)> = found
        .units
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
        .attempt("find_units")
        .expect("the attempt left its record");
    let substeps: Vec<&str> = found
        .substeps
        .iter()
        .map(|substep| substep.name.as_str())
        .collect();
    assert_eq!(
        substeps,
        ["list_files", "read_markers", "group_files_by_unit"]
    );
    let listing = &found.substeps[0];
    assert_eq!(listing.measures.get("files"), Some(&8.0));
    assert_eq!(listing.measures.get("ignored"), Some(&0.0));
    assert_eq!(listing.measures.get("skipped"), Some(&2.0));
    assert_eq!(listing.measures.get("foreign"), Some(&1.0));
    assert_eq!(listing.evidence, ["foreign: apps/other"]);
    let reading = &found.substeps[1];
    assert_eq!(reading.measures.get("markers"), Some(&4.0));
    assert_eq!(reading.measures.get("members_only"), Some(&1.0));
    let grouping = &found.substeps[2];
    assert_eq!(grouping.measures.get("units"), Some(&4.0));
    let _ = fs::remove_dir_all(&repository);
    let _ = fs::remove_dir_all(&runs);
}

#[test]
fn each_law_held_in_a_process_of_its_own_finds_what_the_laws_find_in_one() {
    let demo = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/demo"));
    let plan = Plan::fanned(
        "laws",
        &listed(),
        &[
            "load_ecosystems",
            "find_units",
            "read_graph",
            "list_laws",
            "find_law_instances",
        ],
        &[FanOut::of::<LawName, LawFindings>(
            "find_law_instances",
            |law| law.name.clone(),
        )],
    )
    .expect("the tasks wire into a plan");
    let runs = folder("law-runs");

    let (report, store) = runner()
        .run(&plan, seeded(&demo), &runs)
        .expect("the plan is run");

    assert_eq!(*report.outcome(), RunRecord::passed());
    let gathered = store
        .get::<Vec<LawFindings>>()
        .expect("each law's findings are kept");
    let held: Vec<&str> = gathered.iter().map(|law| law.law.as_str()).collect();
    assert_eq!(
        held,
        [
            "economy",
            "cycles",
            "boundaries",
            "folders",
            "licenses",
            "versions",
            "suppressions",
            "cpp",
            "infra",
            "proto",
            "python",
            "rust",
            "typescript"
        ],
        "every core law, then every language's laws, each one attempt"
    );
    let told = |findings: Vec<&Finding>| {
        let mut all: Vec<(String, Option<u32>, Option<String>, String)> = findings
            .into_iter()
            .map(|finding| {
                (
                    finding.file.to_string_lossy().into_owned(),
                    finding.line,
                    finding.rule.clone(),
                    finding.reason.clone(),
                )
            })
            .collect();
        all.sort();
        all
    };
    let in_one = laws::findings(&graphed(&demo.to_string_lossy()).expect("the fixture is read"));
    assert_eq!(
        told(
            gathered
                .iter()
                .flat_map(|law| law.findings.iter())
                .collect()
        ),
        told(in_one.iter().collect()),
        "the laws find the same, held apart or together"
    );
    let _ = fs::remove_dir_all(&runs);
}

#[test]
fn the_dials_and_the_generated_code_are_read_unit_by_unit_and_fixtures_never() {
    let repository = folder("dialed");
    for (path, content) in [
        (
            "Cargo.toml",
            "[workspace]\nmembers = [\"lowered\"]\n\n[workspace.lints.clippy]\nall = \"allow\"\n",
        ),
        (
            "lowered/Cargo.toml",
            "[package]\nname = \"lowered\"\n\n[lints.clippy]\ncmp_owned = \"allow\"\n",
        ),
        ("lowered/src/lib.rs", "pub fn one() {}\n"),
        ("contracts/buf.yaml", "version: v2\n"),
        ("contracts/shop.proto", "syntax = \"proto3\";\n"),
        (
            "tests/fixture/Cargo.toml",
            "[package]\nname = \"fixture\"\n\n[lints.rust]\nwarnings = \"allow\"\n",
        ),
        ("tests/fixture/src/lib.rs", "pub fn two() {}\n"),
    ] {
        let file = repository.join(path);
        fs::create_dir_all(file.parent().expect("a file sits in a folder"))
            .expect("a folder is made");
        fs::write(file, content).expect("a file is written");
    }
    let mut seeds = seeded(&repository);
    seeds
        .put(&GateMode { cold: false })
        .expect("a mode encodes");
    let plan = Plan::new(
        "dials",
        &listed(),
        &[
            "load_ecosystems",
            "find_units",
            "read_dials",
            "check_generated",
        ],
    )
    .expect("the tasks wire into a plan");
    let runs = folder("dial-runs");

    let (report, store) = runner().run(&plan, seeds, &runs).expect("the plan is run");

    assert_eq!(*report.outcome(), RunRecord::passed());
    let dials = store.get::<Dials>().expect("the dials are kept");
    let told: Vec<(String, Option<&str>, &str)> = dials
        .findings
        .iter()
        .map(|finding| {
            (
                finding.file.to_string_lossy().into_owned(),
                finding.rule.as_deref(),
                finding.reason.as_str(),
            )
        })
        .collect();
    assert_eq!(
        told,
        [(
            "lowered/Cargo.toml".to_string(),
            Some("lowered-dial"),
            "lints.clippy.cmp_owned is allow; the bar denies every warning"
        )],
        "a workspace that only lists members and a fixture have no dials of their own"
    );
    let generated = store
        .get::<GeneratedChecks>()
        .expect("the generated checks are kept");
    assert!(generated.outcomes.is_empty(), "nothing here is generated");
    assert_eq!(
        generated.notes,
        ["contracts has no contracts reference; arbiter ledger sign records today's"]
    );
    let _ = fs::remove_dir_all(&repository);
    let _ = fs::remove_dir_all(&runs);
}

#[test]
fn no_two_tasks_the_task_executable_serves_share_a_name() {
    let mut seen = HashSet::new();
    for task in listed() {
        assert!(
            seen.insert(task.name.clone()),
            "two tasks are named {}: a task process finds a body by its name",
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
         any folder under it holding an ecosystem's marker; what a .gitignore leaves out unless \
         git carries it, the ecosystems' build output, third_party, dot-folders, links and any \
         folder below holding its own .arbiter are never read, only named\n  \
         reads   Folder, Vec<Ecosystem>\n  \
         leaves  FoundUnits\n  \
         limits  no time limit, one attempt, computed every run\n\
         \n\
         repair_units\n  \
         every repair arbiter can make, one after another since each rewrites the folder the \
         next reads: the languages mend their own findings, the generated code is written again \
         from its contracts, then each tool that can repair what it finds does, unit by unit; a \
         fixture is never repaired, and a repair that does not succeed is told\n  \
         reads   Folder, FoundUnits, Vec<Ecosystem>, GateMode\n  \
         leaves  Repairs\n  \
         limits  no time limit, one attempt, computed every run\n\
         \n\
         read_graph\n  \
         the units found read into the graph: every file a language handles read for what it \
         defines and uses, each use resolved to the definitions it reaches, written out with \
         each language's readings\n  \
         reads   Folder, FoundUnits\n  \
         leaves  GraphData\n  \
         limits  no time limit, one attempt, computed every run\n\
         \n\
         list_laws\n  \
         every law held over the graph: each of the core's laws, then each language's laws by \
         the language's name\n  \
         reads   nothing\n  \
         leaves  Vec<LawName>\n  \
         limits  no time limit, one attempt, computed every run\n\
         \n\
         find_law_instances\n  \
         one law held over the graph, its languages given back their readings: the instances of \
         one of the core's laws, or of one language's laws\n  \
         reads   GraphData, LawName\n  \
         leaves  LawFindings\n  \
         limits  no time limit, one attempt, computed every run\n\
         \n\
         read_dials\n  \
         each unit's dials as its own config sets them, held to the bar, for every language \
         with a marker in the unit except a marker that only lists members; a unit on a test \
         path is a fixture and has none\n  \
         reads   Folder, FoundUnits\n  \
         leaves  Dials\n  \
         limits  no time limit, one attempt, computed every run\n\
         \n\
         check_generated\n  \
         each unit's generated code compared with what its contracts generate, and its \
         contracts with the image the user signed, by every language that compares any; it \
         rewrites nothing, and a unit on a test path is a fixture and is never compared\n  \
         reads   Folder, FoundUnits, GateMode\n  \
         leaves  GeneratedChecks\n  \
         limits  no time limit, one attempt, computed every run\n\
         \n\
         digest_unit_cones\n  \
         each unit's input cone as one digest: its own files, the files of every unit it stands \
         on through the graph, and every lockfile, each file by its path and content\n  \
         reads   GraphData\n  \
         leaves  Vec<UnitCone>\n  \
         limits  no time limit, one attempt, computed every run\n\
         \n\
         plan_unit_checks\n  \
         each tool of each ecosystem once in every unit of that ecosystem, except a fixture and \
         a unit whose marker only lists members, as the program running it there, handed the \
         configs its words point at and the unit's input cone, with the reader of what it \
         prints and the program printing its root when it has them, and a note for each unit an \
         ecosystem plans nothing in; it starts nothing\n  \
         reads   Folder, FoundUnits, Vec<Ecosystem>, Vec<UnitCone>\n  \
         leaves  PlannedChecks\n  \
         limits  no time limit, one attempt, computed every run\n\
         \n\
         list_check_subprocesses\n  \
         every program the checks run, in their order: each check's own, then the one printing \
         its root when it has one\n  \
         reads   PlannedChecks\n  \
         leaves  Vec<Subprocess>\n  \
         limits  no time limit, one attempt, computed every run\n\
         \n\
         run_subprocess\n  \
         starts one program in its folder, the configs it is handed written in its own folder, \
         keeps all it prints and tells how it ended; an exit other than 0 is told, never \
         failed, and only an exit with 0 is taken from the cache\n  \
         reads   Subprocess\n  \
         leaves  SubprocessExit\n  \
         limits  1800s at most, one attempt, replayed for the same inputs once its cache test \
         accepted the output\n\
         \n\
         read_check_findings\n  \
         what each check printed, read into findings when it has a reader, their files placed \
         from the root printed for them; every check is read even after one could not be\n  \
         reads   Folder, PlannedChecks, Vec<SubprocessExit>\n  \
         leaves  Vec<Finding>\n  \
         limits  no time limit, one attempt, computed every run\n\
         \n\
         account_ledger\n  \
         the ledger's account of the graph: every suppression counted, every third party the \
         lockfiles resolve, and how many of each the signed reference does not hold\n  \
         reads   GraphData\n  \
         leaves  LedgerAccount\n  \
         limits  no time limit, one attempt, computed every run\n"
    );
}

#[test]
fn the_tasks_view_tells_when_a_read_fires_and_every_limit_a_task_sets() {
    let shaped = Task::new(
        "mend",
        |_: &mut TaskContext,
         (_held, _broke, _ended): (In<Vec<File>>, Failed<Unit>, Done<Vec<Unit>>)| {
            Ok::<u8, TaskFailure>(0)
        },
    )
    .within(Duration::from_secs(30))
    .retries(2)
    .cached();
    let tested = Task::new("probe", |_: &mut TaskContext, (): ()| {
        Ok::<u8, TaskFailure>(0)
    })
    .cached_when(|code: &u8| *code == 0);
    let alone = Task::new("seed", |_: &mut TaskContext, (): ()| {
        Ok::<Unit, TaskFailure>(Unit {
            path: PathBuf::new(),
            markers: Vec::new(),
            files: Vec::new(),
        })
    });
    assert_eq!(
        tasks::shown(&[shaped, tested, alone]),
        "mend\n  \
         reads   Vec<File>, Unit when its task failed, Vec<Unit> however its task ended\n  \
         leaves  u8\n  \
         limits  30s at most, up to 3 attempts, replayed for the same inputs\n\
         \n\
         probe\n  \
         reads   nothing\n  \
         leaves  u8\n  \
         limits  no time limit, one attempt, replayed for the same inputs once its cache test \
         accepted the output\n\
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
                "has modules",
                "format_proto, lint_proto",
                "npx",
                "none",
                "none"
            ],
            vec![
                "python",
                "pyproject.toml",
                "has tool.uv.workspace, lacks project",
                "format_python, lint_python, check_python_types, test_python",
                "uv",
                "uv.lock",
                "__pycache__"
            ],
            vec![
                "rust",
                "Cargo.toml",
                "has workspace, lacks package",
                "format_rust, lint_rust, test_rust",
                "cargo",
                "Cargo.lock",
                "target"
            ],
            vec![
                "typescript",
                "package.json",
                "has workspaces, lacks main, exports, bin",
                "lint_typescript, check_typescript_types, test_typescript",
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

fn seeded(repository: &Path) -> RunStore {
    let mut seeds = RunStore::new();
    seeds
        .put(&Folder {
            path: repository.to_path_buf(),
        })
        .expect("a folder encodes");
    seeds
}

// The tasks named, run_subprocess run once per program listed.
fn fanned(name: &str, tasks: &[&str]) -> Plan {
    Plan::fanned(
        name,
        &listed(),
        tasks,
        &[FanOut::of::<Subprocess, SubprocessExit>(
            "run_subprocess",
            |subprocess| subprocess.name.clone(),
        )],
    )
    .expect("the tasks wire into a plan")
}

// Each program listed, by its name, beside how it ended and all it printed.
fn ran(store: &RunStore) -> Vec<(String, SubprocessExit)> {
    let listed = store
        .get::<Vec<Subprocess>>()
        .expect("the programs are listed");
    let exits = store
        .get::<Vec<SubprocessExit>>()
        .expect("the exits are gathered");
    listed
        .into_iter()
        .map(|subprocess| subprocess.name)
        .zip(exits)
        .collect()
}

// Each finding as its check, its path, its rule and its line.
fn found(findings: &[Finding]) -> Vec<(&str, &str, Option<&str>, Option<u32>)> {
    findings
        .iter()
        .map(|finding| {
            (
                finding.check.as_str(),
                finding.file.to_str().expect("a path in UTF-8"),
                finding.rule.as_deref(),
                finding.line,
            )
        })
        .collect()
}

#[test]
fn the_tools_an_ecosystem_declares_are_run_in_every_unit_each_program_as_an_item_of_its_own() {
    let repository = repository(
        "tools",
        &[
            ("Cargo.toml", "[package]\n"),
            ("src/lib.rs", "pub fn one() {}\n"),
            ("libs/a/Cargo.toml", "[package]\nname = \"a\"\n"),
            ("libs/a/src/lib.rs", "pub fn a() {}\n"),
        ],
    );
    let mut ecosystems = load_ecosystems().expect("the records are read");
    for tool in ecosystems
        .iter_mut()
        .flat_map(|ecosystem| ecosystem.tools.iter_mut())
    {
        let words: &[&str] = match tool.name.as_str() {
            "format_rust" => &["sh", "-c", "echo unformatted; exit 1"],
            "lint_rust" => &["true"],
            "test_rust" => &["cat", "Cargo.toml"],
            _ => continue,
        };
        tool.run = words.iter().map(|word| word.to_string()).collect();
        tool.findings = None;
    }
    let mut seeds = seeded(&repository);
    seeds.put(&ecosystems).expect("the ecosystems encode");
    let plan = fanned(
        "tools",
        &[
            "find_units",
            "read_graph",
            "digest_unit_cones",
            "plan_unit_checks",
            "list_check_subprocesses",
            "run_subprocess",
            "read_check_findings",
        ],
    );
    let runs = folder("runs");

    let (report, store) = runner().run(&plan, seeds, &runs).expect("the plan is run");

    assert_eq!(*report.outcome(), RunRecord::passed());
    let ran = ran(&store);
    assert_eq!(
        ran.iter()
            .map(|(name, exit)| (name.as_str(), exit.code))
            .collect::<Vec<_>>(),
        [
            ("format_rust in .", Some(1)),
            ("format_rust in libs/a", Some(1)),
            ("lint_rust in .", Some(0)),
            ("lint_rust in libs/a", Some(0)),
            ("test_rust in .", Some(0)),
            ("test_rust in libs/a", Some(0)),
        ],
        "every program is run, its exit gathered in the order listed"
    );
    assert_eq!(
        ran[5].1.stdout, "[package]\nname = \"a\"\n",
        "all a program printed is in its exit"
    );
    let items = &report
        .invocation("run_subprocess")
        .expect("the programs ran")
        .instances;
    assert!(
        items.iter().all(|item| item
            .attempts
            .last()
            .is_some_and(|attempt| attempt.process_record.is_some())),
        "each program ran in a process of its own"
    );
    let findings = store.get::<Vec<Finding>>().expect("the findings are kept");
    assert!(
        findings.is_empty(),
        "a program with no reader is told by how it ended, never as a finding"
    );
    assert_eq!(
        items
            .iter()
            .map(|item| item.id().path.as_str())
            .collect::<Vec<_>>(),
        [
            "tools/run_subprocess@format_rust in .",
            "tools/run_subprocess@format_rust in libs/a",
            "tools/run_subprocess@lint_rust in .",
            "tools/run_subprocess@lint_rust in libs/a",
            "tools/run_subprocess@test_rust in .",
            "tools/run_subprocess@test_rust in libs/a",
        ],
        "the record a finding points at goes by the name of its program"
    );
    let _ = fs::remove_dir_all(&repository);
    let _ = fs::remove_dir_all(&runs);
}

// The programs of the run taken from the cache, by name.
fn replayed(report: &RunReport, plan: &str) -> Vec<String> {
    let prefix = format!("{plan}/run_subprocess@");
    report
        .invocation("run_subprocess")
        .expect("the programs ran")
        .instances
        .iter()
        .filter(|item| item.attempts.last().is_some_and(|attempt| attempt.replayed))
        .map(|item| item.id().path.trim_start_matches(&prefix).to_string())
        .collect()
}

#[test]
fn a_program_that_exited_with_0_is_taken_from_the_cache_until_its_unit_s_cone_moves_and_any_other_runs_again(
) {
    let repository = repository(
        "cached",
        &[
            (
                "Cargo.toml",
                "[package]\nname = \"top\"\n\n[dependencies]\na = { path = \"libs/a\" }\n",
            ),
            ("src/lib.rs", "use a::a;\n\npub fn one() {\n    a();\n}\n"),
            ("libs/a/Cargo.toml", "[package]\nname = \"a\"\n"),
            ("libs/a/src/lib.rs", "pub fn a() {}\n"),
            ("libs/b/Cargo.toml", "[package]\nname = \"b\"\n"),
            ("libs/b/src/lib.rs", "pub fn b() {}\n"),
        ],
    );
    let mut ecosystems = load_ecosystems().expect("the records are read");
    for tool in ecosystems
        .iter_mut()
        .flat_map(|ecosystem| ecosystem.tools.iter_mut())
    {
        let words: &[&str] = match tool.name.as_str() {
            "format_rust" | "test_rust" => &["true"],
            "lint_rust" => &["sh", "-c", "exit 1"],
            _ => continue,
        };
        tool.run = words.iter().map(|word| word.to_string()).collect();
        tool.findings = None;
    }
    let plan = fanned(
        "cached",
        &[
            "find_units",
            "read_graph",
            "digest_unit_cones",
            "plan_unit_checks",
            "list_check_subprocesses",
            "run_subprocess",
        ],
    );
    let runs = folder("runs");
    let run = || {
        let mut seeds = seeded(&repository);
        seeds.put(&ecosystems).expect("the ecosystems encode");
        let (report, _) = runner().run(&plan, seeds, &runs).expect("the plan is run");
        report
    };

    assert!(
        replayed(&run(), "cached").is_empty(),
        "a first run computes all"
    );
    assert_eq!(
        replayed(&run(), "cached"),
        [
            "format_rust in .",
            "format_rust in libs/a",
            "format_rust in libs/b",
            "test_rust in .",
            "test_rust in libs/a",
            "test_rust in libs/b",
        ],
        "an exit with 0 is taken from the cache, and the lint exiting with 1 runs again"
    );
    fs::write(
        repository.join("libs/a/src/lib.rs"),
        "pub fn a() {}\n\npub fn more() {}\n",
    )
    .expect("the file is rewritten");
    assert_eq!(
        replayed(&run(), "cached"),
        ["format_rust in libs/b", "test_rust in libs/b"],
        "a changed file runs again the programs of its unit and of every unit standing on it"
    );
    let _ = fs::remove_dir_all(&repository);
    let _ = fs::remove_dir_all(&runs);
}

// The gate's tasks over a folder whose ecosystems are seeded, in plan order.
const GATE: [&str; 12] = [
    "find_units",
    "read_graph",
    "list_laws",
    "find_law_instances",
    "read_dials",
    "check_generated",
    "digest_unit_cones",
    "plan_unit_checks",
    "list_check_subprocesses",
    "run_subprocess",
    "read_check_findings",
    "account_ledger",
];

fn gate_plan() -> Plan {
    Plan::fanned(
        "gate",
        &listed(),
        &GATE,
        &[
            FanOut::of::<LawName, LawFindings>("find_law_instances", |law| law.name.clone()),
            FanOut::of::<Subprocess, SubprocessExit>("run_subprocess", |subprocess| {
                subprocess.name.clone()
            }),
        ],
    )
    .expect("the tasks wire into a plan")
}

#[test]
fn the_report_is_written_from_the_values_the_plan_left_and_the_record_of_its_run() {
    let repository = repository(
        "reported",
        &[
            ("Cargo.toml", "[package]\nname = \"top\"\n"),
            ("src/lib.rs", "pub fn one() {}\n"),
            ("libs/a/Cargo.toml", "[package]\nname = \"a\"\n"),
            ("libs/a/src/lib.rs", "pub fn a() {}\n"),
        ],
    );
    let mut ecosystems = load_ecosystems().expect("the records are read");
    for tool in ecosystems
        .iter_mut()
        .flat_map(|ecosystem| ecosystem.tools.iter_mut())
    {
        let words: &[&str] = match tool.name.as_str() {
            "format_rust" => &["sh", "-c", "echo unformatted; exit 1"],
            "lint_rust" | "test_rust" => &["true"],
            _ => continue,
        };
        tool.run = words.iter().map(|word| word.to_string()).collect();
        tool.findings = None;
    }
    let mut seeds = seeded(&repository);
    seeds.put(&ecosystems).expect("the ecosystems encode");
    seeds
        .put(&GateMode { cold: false })
        .expect("the mode encodes");
    let tasks = GATE;
    let plan = gate_plan();
    let runs = folder("runs");

    let (record, store) = runner().run(&plan, seeds, &runs).expect("the plan is run");
    let run = of_plan::gate_run(&store, &record).expect("every value is left");

    assert_eq!(
        run.outcomes
            .iter()
            .map(|outcome| (
                outcome.check.as_str(),
                outcome.unit.to_str().expect("a path in UTF-8"),
                outcome.language.as_str(),
                outcome.passed
            ))
            .collect::<Vec<_>>(),
        [
            ("format_rust", "", "rust", false),
            ("lint_rust", "", "rust", true),
            ("test_rust", "", "rust", true),
            ("format_rust", "libs/a", "rust", false),
            ("lint_rust", "libs/a", "rust", true),
            ("test_rust", "libs/a", "rust", true),
        ],
        "unit by unit, each unit's tools in the order planned"
    );
    assert_eq!(run.outcomes[0].output, "unformatted\n");
    assert!(
        run.outcomes
            .iter()
            .all(|outcome| outcome.seconds > 0.0 && !outcome.warm),
        "each program's time is read from the record of its run"
    );
    let graph = graphed(&repository.to_string_lossy()).expect("the folder is read");
    let held = laws::findings(&graph);
    assert!(
        held.iter().all(|finding| run.findings.contains(finding)),
        "every finding of the laws is reported"
    );
    assert!(
        run.findings
            .iter()
            .all(|finding| finding.check != "format_rust"),
        "a check that failed is told by its outcome alone"
    );
    assert!(
        run.findings
            .windows(2)
            .all(|pair| (&pair[0].file, pair[0].line) <= (&pair[1].file, pair[1].line)),
        "in the order of file and line"
    );
    assert_eq!(run.ledger.counted.len(), 0);
    assert_eq!(
        run.steps
            .iter()
            .map(|step| step.op.as_str())
            .collect::<Vec<_>>(),
        tasks,
        "the steps are the plan's tasks"
    );
    let programs = &run.steps[9];
    assert_eq!(
        programs
            .children
            .iter()
            .map(|item| item.op.as_str())
            .collect::<Vec<_>>(),
        [
            "format_rust in .",
            "format_rust in libs/a",
            "lint_rust in .",
            "lint_rust in libs/a",
            "test_rust in .",
            "test_rust in libs/a",
        ],
        "a task run per item holds each item under it"
    );
    assert!(files::report(&run.outcomes, &run.findings)
        .contains("\n## format_rust — libs/a\n\nunformatted\n"));
    let _ = fs::remove_dir_all(&repository);
    let _ = fs::remove_dir_all(&runs);
}

#[test]
fn a_fixing_run_repairs_one_thing_at_a_time_before_the_gate_and_a_failed_repair_stands_for_its_check(
) {
    let repository = repository(
        "repaired",
        &[
            ("Cargo.toml", "[package]\nname = \"top\"\n"),
            (
                "src/lib.rs",
                "use std::{fmt, io};\n\npub fn one(_: &dyn fmt::Debug, _: io::ErrorKind) {}\n",
            ),
            ("libs/a/Cargo.toml", "[package]\nname = \"a\"\n"),
            ("libs/a/src/lib.rs", "pub fn a() {}\n"),
            (
                "tests/fixture/Cargo.toml",
                "[package]\nname = \"fixture\"\n",
            ),
            ("tests/fixture/src/lib.rs", "use std::{fmt, io};\n"),
        ],
    );
    let runs = folder("runs");
    let order = runs.join("order");
    let shell = |script: String| vec!["sh".to_string(), "-c".to_string(), script];
    let mut ecosystems = load_ecosystems().expect("the records are read");
    for tool in ecosystems
        .iter_mut()
        .flat_map(|ecosystem| ecosystem.tools.iter_mut())
    {
        let repair = match tool.name.as_str() {
            "format_rust" => format!("echo format $(pwd -P) >> {}", order.display()),
            "lint_rust" => format!(
                "echo lint $(pwd -P) >> {}; echo broken; exit 3",
                order.display()
            ),
            "test_rust" => {
                tool.run = vec!["true".to_string()];
                continue;
            }
            _ => continue,
        };
        tool.run = vec!["true".to_string()];
        tool.fix = Some(shell(repair));
        tool.findings = None;
    }
    let seeds = || {
        let mut seeds = seeded(&repository);
        seeds.put(&ecosystems).expect("the ecosystems encode");
        seeds
            .put(&GateMode { cold: false })
            .expect("the mode encodes");
        seeds
    };
    let repair = Plan::new("repair", &listed(), &["find_units", "repair_units"])
        .expect("the tasks wire into a plan");

    let (record, store) = runner()
        .run(&repair, seeds(), &runs)
        .expect("the repairs are run");

    assert_eq!(*record.outcome(), RunRecord::passed());
    let here = fs::canonicalize(&repository).expect("the folder resolves");
    let here = here.display();
    assert_eq!(
        fs::read_to_string(&order)
            .expect("the repairs ran")
            .lines()
            .collect::<Vec<_>>(),
        [
            format!("format {here}"),
            format!("lint {here}"),
            format!("format {here}/libs/a"),
            format!("lint {here}/libs/a"),
        ],
        "one at a time, unit by unit, each unit's tools in the order its record writes them"
    );
    let repairs = store.get::<Repairs>().expect("the repairs are told");
    assert_eq!(repairs.notes, ["src/lib.rs split to one use per line"]);
    assert!(fs::read_to_string(repository.join("src/lib.rs"))
        .expect("readable")
        .starts_with("use std::fmt;\nuse std::io;\n"));
    assert_eq!(
        fs::read_to_string(repository.join("tests/fixture/src/lib.rs")).expect("readable"),
        "use std::{fmt, io};\n",
        "a fixture is never repaired"
    );
    assert_eq!(
        repairs
            .failed
            .iter()
            .map(|failed| (
                failed.check.as_str(),
                failed.unit.to_str().expect("a path in UTF-8"),
                failed.output.as_str()
            ))
            .collect::<Vec<_>>(),
        [
            ("lint_rust", "", "broken\n"),
            ("lint_rust", "libs/a", "broken\n")
        ]
    );

    let (record, store) = runner()
        .run(&gate_plan(), seeds(), &runs)
        .expect("the gate is run");
    let run = of_plan::gate_run(&store, &record)
        .expect("every value is left")
        .repaired(repairs);

    assert_eq!(
        run.outcomes
            .iter()
            .map(|outcome| (
                outcome.check.as_str(),
                outcome.unit.to_str().expect("a path in UTF-8"),
                outcome.passed,
                outcome.output.as_str()
            ))
            .collect::<Vec<_>>(),
        [
            ("format_rust", "", true, ""),
            ("lint_rust", "", false, "broken\n"),
            ("test_rust", "", true, ""),
            ("format_rust", "libs/a", true, ""),
            ("lint_rust", "libs/a", false, "broken\n"),
            ("test_rust", "libs/a", true, ""),
        ],
        "the failed repair stands for its check, which passed on the repaired files"
    );
    assert_eq!(
        run.notes.last().map(String::as_str),
        Some("src/lib.rs split to one use per line"),
        "the repairs' notes follow the gate's own"
    );
    let _ = fs::remove_dir_all(&repository);
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
            ("globbed/Cargo.toml", &alone("globbed")),
            (
                "globbed/src/lib.rs",
                "mod deps;\n\nuse deps::*;\n\npub fn run() -> u8 {\n    used()\n}\n",
            ),
            ("globbed/src/deps.rs", "pub fn used() -> u8 {\n    1\n}\n"),
            ("solo/Cargo.toml", &alone("solo")),
            (
                "solo/src/lib.rs",
                "pub fn same(x: u32) -> bool{\n    x == x\n}\n",
            ),
        ],
    );
    let plan = fanned(
        "gate",
        &[
            "load_ecosystems",
            "find_units",
            "read_graph",
            "digest_unit_cones",
            "plan_unit_checks",
            "list_check_subprocesses",
            "run_subprocess",
            "read_check_findings",
        ],
    );
    let runs = folder("runs");

    let (report, store) = runner()
        .run(&plan, seeded(&repository), &runs)
        .expect("the plan is run");

    assert_eq!(*report.outcome(), RunRecord::passed());
    let ran = ran(&store);
    assert_eq!(
        ran.len(),
        16,
        "three tools in each of four units, and a root for each lint: {:?}",
        ran.iter().map(|(name, _)| name).collect::<Vec<_>>()
    );
    let findings = store.get::<Vec<Finding>>().expect("the findings are kept");
    assert_eq!(
        found(&findings),
        [
            (
                "lint_rust",
                "globbed/src/lib.rs",
                Some("clippy::wildcard_imports"),
                Some(3)
            ),
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
        "a member's file is placed from the root of the workspace cargo wrote it from, \
         and a glob import is a fault like any other"
    );
    let told = ran
        .iter()
        .find(|(name, _)| name == "format_rust in solo")
        .map(|(_, exit)| exit.stdout.as_str())
        .expect("the format ran in solo");
    assert!(
        told.contains("bool {"),
        "the tool's own output is kept: {told}"
    );
    let _ = fs::remove_dir_all(&repository);
    let _ = fs::remove_dir_all(&runs);
}
