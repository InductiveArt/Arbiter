use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use task_runner::context::stop_token::StopToken;
use task_runner::contract::run_record::Ended;
use task_runner::contract::RunId;
use task_runner::contract::RunRecord;
use task_runner::contract::RunReport;
use task_runner::contract::TaskAttempt;
use task_runner::declaration::fan_out::FanOut;
use task_runner::declaration::plan::Plan;
use task_runner::execution::event::Event;
use task_runner::execution::observer::Observer;
use task_runner::execution::observer::Quiet;
use task_runner::execution::run_folder::OUTPUTS_DIR;
use task_runner::execution::run_folder::RUNNING_FILE;
use task_runner::execution::run_folder::RUNS_DIR;
use task_runner::execution::run_folder::RUN_FILE;
use task_runner::execution::runner::Runner;
use task_runner::protocol::attempt_folder::RECORD_FILE;
use task_runner::protocol::attempt_folder::STDERR_FILE;
use task_runner::protocol::attempt_folder::STDOUT_FILE;
use task_runner::record::refusal::RefusalReason;
use task_runner::test_tasks::tasks::listed;
use task_runner::test_tasks::values::Answered;
use task_runner::test_tasks::values::Appetite;
use task_runner::test_tasks::values::Chatter;
use task_runner::test_tasks::values::Costly;
use task_runner::test_tasks::values::Flaked;
use task_runner::test_tasks::values::Flakiness;
use task_runner::test_tasks::values::Graph;
use task_runner::test_tasks::values::Held;
use task_runner::test_tasks::values::Licenses;
use task_runner::test_tasks::values::Number;
use task_runner::test_tasks::values::Repo;
use task_runner::test_tasks::values::Reverted;
use task_runner::test_tasks::values::Root;
use task_runner::test_tasks::values::Said;
use task_runner::test_tasks::values::Source;
use task_runner::test_tasks::values::Squared;
use task_runner::test_tasks::values::Summary;
use task_runner::test_tasks::values::Told;
use task_runner::test_tasks::values::Total;
use task_runner::test_tasks::values::Turns;
use task_runner::test_tasks::values::Unsummed;
use task_runner::value::run_store::RunStore;
use task_runner::value::task_output_hash::TaskOutputHash;
use task_runner::value::value_type::ValueType;

fn runner() -> Runner {
    Runner::new().task_executable(PathBuf::from(env!("CARGO_BIN_EXE_test-tasks")))
}

fn root(name: &str) -> PathBuf {
    let dir = env::temp_dir().join(format!(
        "task-runner-process-{name}-{}-{}",
        std::process::id(),
        RunId::next()
    ));
    fs::create_dir_all(&dir).expect("a root is made");
    dir
}

fn gate() -> Plan {
    Plan::new(
        "gate",
        &listed(),
        &["scan", "resolve", "cycles", "boundaries", "tools", "ledger"],
    )
    .expect("the gate test plan is valid")
}

fn repair() -> Plan {
    Plan::new(
        "gate",
        &listed(),
        &["fix", "check", "commit", "revert", "archive", "report"],
    )
    .expect("the repair test plan is valid")
}

fn seeded(name: &str, parses: bool) -> RunStore {
    let mut store = RunStore::new();
    store
        .put(&Root {
            name: name.to_string(),
            parses,
        })
        .expect("a root encodes");
    store
}

fn sourced(compiles: bool) -> RunStore {
    let mut store = RunStore::new();
    store
        .put(&Source {
            name: "lib.rs".to_string(),
            compiles,
        })
        .expect("a source encodes");
    store
}

// The named task run once per number, its squares summed, and the reason
// noted when they could not all be had.
fn squares(square: &str) -> Plan {
    Plan::fanned(
        "gate",
        &listed(),
        &[square, "sum", "unsummed"],
        &[FanOut::of::<Number, Squared>(square, |number| {
            number.value.to_string()
        })],
    )
    .expect("the squares test plan is valid")
}

fn numbered(values: &[i64]) -> RunStore {
    let numbers: Vec<Number> = values.iter().map(|&value| Number { value }).collect();
    let mut store = RunStore::new();
    store.put(&numbers).expect("the numbers encode");
    store
}

fn outcome_of(report: &RunReport, name: &str) -> Option<RunRecord> {
    report
        .attempt(name)
        .map(|attempt| attempt.outcome().clone())
}

fn seconds_between(a: std::time::SystemTime, b: std::time::SystemTime) -> f64 {
    match a.duration_since(b) {
        Ok(gap) => gap.as_secs_f64(),
        Err(earlier) => earlier.duration().as_secs_f64(),
    }
}

#[test]
fn a_task_starts_only_once_every_type_it_reads_is_produced() {
    let root = root("order");
    // Declared in reverse, so the order can come only from the data.
    let plan = Plan::new(
        "gate",
        &listed(),
        &["ledger", "tools", "boundaries", "cycles", "resolve", "scan"],
    )
    .expect("the plan is valid");
    let (report, store) = runner()
        .run(&plan, seeded("repo", true), &root)
        .expect("the root is seeded");
    assert_eq!(*report.outcome(), RunRecord::passed());
    assert_eq!(
        store.get::<Summary>().map(|summary| summary.line),
        Some("41 42 clippy+test".to_string())
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn independent_tasks_run_at_the_same_time() {
    let root = root("parallel");
    let (report, _) = runner()
        .run(&gate(), seeded("repo", true), &root)
        .expect("the root is seeded");
    assert_eq!(*report.outcome(), RunRecord::passed());
    let cycles = report
        .attempt("cycles")
        .and_then(|attempt| attempt.began())
        .expect("began");
    let bounds = report
        .attempt("boundaries")
        .and_then(|attempt| attempt.began())
        .expect("began");
    assert!(
        seconds_between(cycles, bounds) < 0.15,
        "two independent 200ms tasks began {}s apart",
        seconds_between(cycles, bounds)
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn one_slot_runs_the_tasks_one_after_the_other() {
    let root = root("serial");
    let (report, _) = runner()
        .slots(1)
        .run(&gate(), seeded("repo", true), &root)
        .expect("the root is seeded");
    assert_eq!(*report.outcome(), RunRecord::passed());
    let cycles = report
        .attempt("cycles")
        .and_then(|attempt| attempt.began())
        .expect("began");
    let bounds = report
        .attempt("boundaries")
        .and_then(|attempt| attempt.began())
        .expect("began");
    assert!(
        seconds_between(cycles, bounds) >= 0.2,
        "two 200ms tasks on one slot began {}s apart: they overlapped",
        seconds_between(cycles, bounds)
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn every_task_keeps_its_own_invocation_in_plan_order_with_path_ids() {
    let root = root("ids");
    let (report, _) = runner()
        .run(&gate(), seeded("repo", true), &root)
        .expect("the root is seeded");
    let ids: Vec<&str> = report
        .invocations
        .iter()
        .map(|invocation| invocation.id().path.as_str())
        .collect();
    assert_eq!(
        ids,
        [
            "gate/scan",
            "gate/resolve",
            "gate/cycles",
            "gate/boundaries",
            "gate/tools",
            "gate/ledger"
        ]
    );
    let tools = report.attempt("tools").expect("the tools task ran");
    let substeps: Vec<&str> = tools
        .substeps
        .iter()
        .map(|substep| substep.name.as_str())
        .collect();
    assert_eq!(substeps, ["clippy", "test"]);
    let scan = report.attempt("scan").expect("the scan task ran");
    assert_eq!(scan.evidence, ["scanned repo"]);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_run_has_its_own_id_and_every_invocation_carries_it() {
    let root = root("runs");
    let (one, _) = runner()
        .run(&gate(), seeded("repo", true), &root)
        .expect("the root is seeded");
    let (two, _) = runner()
        .run(&gate(), seeded("repo", true), &root)
        .expect("the root is seeded");
    assert_ne!(one.run(), two.run());
    assert!(one
        .invocations
        .iter()
        .all(|invocation| invocation.id().run() == one.run()));
    assert!(two
        .invocations
        .iter()
        .all(|invocation| invocation.id().run() == two.run()));
    let scan = one.attempt("scan").expect("the scan task ran");
    assert_eq!(scan.id().to_string(), format!("{}/gate/scan#1", one.run()));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn an_attempt_records_its_slot_and_the_time_it_waited_for_one() {
    let root = root("slots");
    let plan = Plan::new("gate", &listed(), &["cycles", "boundaries"]).expect("valid");
    let mut seeds = RunStore::new();
    seeds.put(&Graph { nodes: 4 }).expect("encodes");
    let (report, _) = runner()
        .slots(1)
        .run(&plan, seeds, &root)
        .expect("the graph is seeded");
    let first = report.attempt("cycles").expect("ran");
    let second = report.attempt("boundaries").expect("ran");
    assert_eq!((first.slot, second.slot), (Some(0), Some(0)));
    let first_wait = first.wait().expect("an attempt records its wait");
    let second_wait = second.wait().expect("an attempt records its wait");
    assert!(
        first_wait < Duration::from_millis(100),
        "the first task waited {first_wait:?} with a free slot"
    );
    assert!(
        second_wait >= Duration::from_millis(150),
        "the second task waited {second_wait:?} behind a 200ms task on one slot"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn an_attempt_s_record_carries_the_kernel_s_account_of_its_process() {
    let root = root("account");
    let plan = Plan::new("gate", &listed(), &["hungry"]).expect("valid");
    let mut seeds = RunStore::new();
    seeds.put(&Appetite { mebibytes: 64 }).expect("encodes");
    let (report, store) = runner()
        .run(&plan, seeds, &root)
        .expect("the appetite is seeded");
    assert_eq!(*report.outcome(), RunRecord::passed());
    assert_eq!(store.get::<Held>().map(|held| held.bytes), Some(64 << 20));
    let attempt = report.attempt("hungry").expect("ran");
    let process_record = attempt
        .process_record
        .as_ref()
        .expect("an attempt has an account");
    assert_eq!(
        (
            process_record.exit_code,
            process_record.signal,
            process_record.killed
        ),
        (Some(0), None, false)
    );
    assert!(
        process_record.usage().max_rss_bytes >= 64 << 20,
        "a process holding 64 MiB peaked at {} bytes",
        process_record.usage().max_rss_bytes
    );
    assert!(process_record.usage().cpu() > Duration::ZERO);
    assert!(
        process_record.usage().minor_faults > 0,
        "touching 64 MiB faults pages in"
    );
    let startup = process_record
        .startup()
        .expect("the task process marked when its body began");
    assert!(
        startup < process_record.wall(),
        "startup {startup:?} within a wall of {:?}",
        process_record.wall()
    );
    assert!(
        attempt.resources().cpu().is_some(),
        "the attempt measured its own thread too"
    );
    assert!(process_record.wall() >= attempt.resources().wall());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_report_sums_the_kernel_s_account_of_every_attempt() {
    let root = root("bill");
    let plan = Plan::new("gate", &listed(), &["flaky"]).expect("valid");
    let mut seeds = RunStore::new();
    seeds.put(&Flakiness { passes_at: 2 }).expect("encodes");
    let (report, _) = runner()
        .run(&plan, seeds, &root)
        .expect("the flakiness is seeded");
    assert_eq!(*report.outcome(), RunRecord::passed());
    let attempts = report.attempts_of("flaky");
    assert_eq!(attempts.len(), 2);
    let billed: Duration = attempts
        .iter()
        .filter_map(|attempt| attempt.process_record.as_ref())
        .map(|process_record| process_record.usage().cpu())
        .sum();
    assert_eq!(report.usage().cpu(), billed);
    assert!(report.usage().cpu() > Duration::ZERO);
    let peak = attempts
        .iter()
        .filter_map(|attempt| attempt.process_record.as_ref())
        .map(|process_record| process_record.usage().max_rss_bytes)
        .max()
        .expect("accounted");
    assert_eq!(report.usage().max_rss_bytes, peak);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_failed_task_skips_every_task_standing_on_it_and_the_rest_still_run() {
    let root = root("skip");
    let plan = Plan::new(
        "gate",
        &listed(),
        &[
            "scan",
            "resolve",
            "cycles",
            "boundaries",
            "tools",
            "ledger",
            "licenses",
        ],
    )
    .expect("the plan is valid");
    let (report, store) = runner()
        .run(&plan, seeded("broken", false), &root)
        .expect("the root is seeded");
    assert!(matches!(report.outcome().ended, Some(Ended::Failed(_))));
    assert_eq!(outcome_of(&report, "scan"), Some(RunRecord::passed()));
    assert_eq!(
        outcome_of(&report, "resolve"),
        Some(RunRecord::failed("the parser choked".to_string()))
    );
    assert_eq!(
        outcome_of(&report, "cycles"),
        Some(RunRecord::skipped("gate/resolve".to_string()))
    );
    assert_eq!(
        outcome_of(&report, "ledger"),
        Some(RunRecord::skipped("gate/resolve".to_string()))
    );
    assert_eq!(outcome_of(&report, "licenses"), Some(RunRecord::passed()));
    assert_eq!(store.get::<Licenses>().map(|found| found.bytes), Some(6));
    let cycles = report
        .attempt("cycles")
        .expect("the skipped task has a record");
    assert_eq!(cycles.id().attempt, 0, "a skipped task never attempted");
    assert!(cycles.began().is_none() && cycles.process_record.is_none());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_body_that_panics_fails_its_attempt_and_the_rest_of_the_run_goes_on() {
    let root = root("panic");
    let plan = Plan::new("gate", &listed(), &["scan", "boom"]).expect("the plan is valid");
    let (report, _) = runner()
        .run(&plan, seeded("repo", true), &root)
        .expect("the root is seeded");
    assert!(matches!(report.outcome().ended, Some(Ended::Failed(_))));
    assert_eq!(outcome_of(&report, "scan"), Some(RunRecord::passed()));
    let boom = report.attempt("boom").expect("recorded");
    match &boom.outcome().ended {
        Some(Ended::Failed(reason)) => {
            assert!(reason.contains("exited with code 101"), "{reason}");
            assert!(reason.contains("boom"), "{reason}");
        }
        other => panic!("a panic did not fail the attempt: {other:?}"),
    }
    let process_record = boom.process_record.as_ref().expect("accounted");
    assert_eq!(process_record.exit_code, Some(101));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_failed_attempt_is_tried_again_up_to_the_retries_and_each_keeps_its_record() {
    let root = root("retries");
    let plan = Plan::new("gate", &listed(), &["flaky"]).expect("the plan is valid");
    let mut seeds = RunStore::new();
    seeds.put(&Flakiness { passes_at: 3 }).expect("encodes");
    let (report, store) = runner()
        .run(&plan, seeds, &root)
        .expect("the flakiness is seeded");
    assert_eq!(*report.outcome(), RunRecord::passed());
    assert_eq!(store.get::<Flaked>().map(|flaked| flaked.attempt), Some(3));
    let attempts: Vec<(u32, RunRecord)> = report
        .attempts_of("flaky")
        .iter()
        .map(|recorded| (recorded.id().attempt, recorded.outcome().clone()))
        .collect();
    assert_eq!(
        attempts,
        [
            (1, RunRecord::failed("attempt 1 failed".to_string())),
            (2, RunRecord::failed("attempt 2 failed".to_string())),
            (3, RunRecord::passed())
        ]
    );
    for attempt in 1..=3 {
        assert!(
            report
                .dir()
                .join("gate")
                .join(format!("flaky#{attempt}"))
                .join(RECORD_FILE)
                .is_file(),
            "attempt {attempt} left its record on disk"
        );
    }
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_failure_read_fires_only_once_the_retries_are_spent() {
    let root = root("spent");
    let plan = Plan::new("gate", &listed(), &["never", "answer"]).expect("the plan is valid");
    let (report, store) = runner()
        .run(&plan, seeded("repo", true), &root)
        .expect("the root is seeded");
    assert_eq!(report.attempts_of("never").len(), 2);
    assert_eq!(report.attempts_of("answer").len(), 1);
    assert_eq!(
        store.get::<Answered>().map(|answered| answered.line),
        Some("gate/never#2".to_string())
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_task_out_of_time_is_timed_out_and_its_process_tree_ended_so_the_slot_is_freed() {
    let root = root("timeout");
    let plan = Plan::new("gate", &listed(), &["sleeper", "quick"]).expect("the plan is valid");
    let (report, _) = runner()
        .slots(1)
        .run(&plan, seeded("repo", true), &root)
        .expect("the root is seeded");
    assert!(
        report.seconds < 3.0,
        "the run took {}s: the sleeping command held the slot",
        report.seconds
    );
    assert_eq!(outcome_of(&report, "sleeper"), Some(RunRecord::timed_out()));
    assert_eq!(outcome_of(&report, "quick"), Some(RunRecord::passed()));
    assert!(matches!(report.outcome().ended, Some(Ended::Failed(_))));
    let sleeper = report.attempt("sleeper").expect("recorded");
    let process_record = sleeper.process_record.as_ref().expect("accounted");
    assert!(process_record.killed, "the run stopped it");
    assert!(
        process_record.wall() < Duration::from_secs(2),
        "the task process lived {:?} past a 100ms allowance",
        process_record.wall()
    );
    let command = sleeper
        .resources()
        .processes
        .first()
        .expect("the task process recorded the command it ran before it stopped");
    assert_eq!(
        command.exit_code, None,
        "the five second sleep did not end on its own"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_body_that_checks_in_stops_at_its_next_checkpoint_once_out_of_time() {
    let root = root("checkpoint");
    let plan = Plan::new("gate", &listed(), &["looper", "quick"]).expect("the plan is valid");
    let (report, store) = runner()
        .slots(1)
        .run(&plan, seeded("repo", true), &root)
        .expect("the root is seeded");
    assert!(report.seconds < 2.0, "the run took {}s", report.seconds);
    assert_eq!(outcome_of(&report, "looper"), Some(RunRecord::timed_out()));
    assert_eq!(outcome_of(&report, "quick"), Some(RunRecord::passed()));
    assert!(store.get::<Turns>().is_none());
    let looper = report.attempt("looper").expect("recorded");
    assert!(
        looper.evidence.first().is_some_and(|line| line == "turn 0"),
        "the record the body left on stopping is kept: {:?}",
        looper.evidence
    );
    let process_record = looper.process_record.as_ref().expect("accounted");
    assert_eq!(
        (process_record.signal, process_record.exit_code),
        (None, Some(1)),
        "a body that stopped at its checkpoint ended on its own"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_body_deaf_to_the_request_to_stop_is_killed_once_the_grace_is_spent() {
    let root = root("deaf");
    let plan = Plan::new("gate", &listed(), &["deaf"]).expect("the plan is valid");
    let (report, _) = runner()
        .grace(Duration::from_millis(100))
        .run(&plan, seeded("repo", true), &root)
        .expect("the root is seeded");
    assert!(report.seconds < 2.0, "the run took {}s", report.seconds);
    assert_eq!(outcome_of(&report, "deaf"), Some(RunRecord::timed_out()));
    let deaf = report.attempt("deaf").expect("recorded");
    let process_record = deaf.process_record.as_ref().expect("accounted");
    assert_eq!(process_record.signal, Some(9), "it was killed");
    assert!(process_record.killed);
    assert!(
        deaf.began().is_some(),
        "the run knows when it began even without a record"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn cancelling_a_run_stops_what_is_out_and_about_to_go_out_while_a_failure_s_readers_still_answer() {
    let root = root("cancel");
    let plan = Plan::new(
        "gate",
        &listed(),
        &["slow", "later", "after", "undo", "tell"],
    )
    .expect("valid");
    let cancel = StopToken::new();
    let token = cancel.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(300));
        token.stop();
    });
    let (report, store) = runner()
        .slots(1)
        .run_with(&plan, seeded("repo", true), &root, &cancel, &mut Quiet)
        .expect("the root is seeded");
    assert!(report.seconds < 2.5, "the run took {}s", report.seconds);
    assert_eq!(outcome_of(&report, "slow"), Some(RunRecord::cancelled()));
    assert_eq!(outcome_of(&report, "later"), Some(RunRecord::cancelled()));
    assert_eq!(
        report.attempt("later").map(|attempt| attempt.id().attempt),
        Some(0),
        "a task about to go out never attempted"
    );
    assert_eq!(
        outcome_of(&report, "after"),
        Some(RunRecord::skipped("gate/slow".to_string()))
    );
    assert_eq!(outcome_of(&report, "undo"), Some(RunRecord::passed()));
    assert_eq!(
        store.get::<Said>().map(|said| said.line),
        Some("cancelled passed".to_string())
    );
    assert!(matches!(report.outcome().ended, Some(Ended::Failed(_))));
    let _ = fs::remove_dir_all(&root);
}

// Writes down each event as one line, so the order can be read back.
struct Heard(Vec<String>);

impl Observer for Heard {
    fn event(&mut self, event: Event<'_>) {
        self.0.push(match event {
            Event::Started { attempt_id, slot } => {
                format!("started {} on {slot}", attempt_id.path())
            }
            Event::Ended { attempt } => {
                format!("ended {} {}", attempt.id().path(), attempt.outcome())
            }
            Event::Settled { path, outcome } => format!("settled {path} {outcome}"),
            Event::Finished { report } => format!("finished {}", report.outcome()),
        })
    }
}

#[test]
fn an_observer_hears_each_start_each_record_each_settling_and_the_finish_in_order() {
    let root = root("observed");
    let mut heard = Heard(Vec::new());
    let (report, _) = runner()
        .run_with(
            &gate(),
            seeded("repo", true),
            &root,
            &StopToken::new(),
            &mut heard,
        )
        .expect("the root is seeded");
    assert_eq!(*report.outcome(), RunRecord::passed());
    let heard = heard.0;
    assert_eq!(
        heard.first().map(String::as_str),
        Some("started gate/scan on 0")
    );
    assert_eq!(heard.last().map(String::as_str), Some("finished passed"));
    for name in ["scan", "resolve", "cycles", "boundaries", "tools", "ledger"] {
        let at = |prefix: &str| {
            heard
                .iter()
                .position(|line| line.starts_with(&format!("{prefix} gate/{name} ")))
                .unwrap_or_else(|| panic!("{name} was not heard to have {prefix}: {heard:#?}"))
        };
        assert!(
            at("started") < at("ended") && at("ended") < at("settled"),
            "{name} was heard out of order: {heard:#?}"
        );
    }
    assert_eq!(heard.len(), 6 * 3 + 1);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_run_refuses_to_start_when_a_read_type_is_neither_produced_nor_seeded() {
    let root = root("unseeded");
    let plan = Plan::new("gate", &listed(), &["scan", "resolve"]).expect("the plan is valid");
    let refused = runner().run(&plan, RunStore::new(), &root);
    assert_eq!(
        refused.err().map(|refused| refused.reasons),
        Some(vec![RefusalReason::Unseeded {
            value_type: ValueType::of::<Root>()
        }])
    );
    assert!(
        fs::read_dir(root.join(RUNS_DIR))
            .map(|entries| entries.count())
            .unwrap_or(0)
            == 0,
        "nothing ran"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_fanned_task_runs_once_per_item_and_its_readers_read_the_outputs_gathered_in_item_order() {
    let root = root("fanned");
    let (report, store) = runner()
        .run(&squares("square"), numbered(&[1, 2, 3]), &root)
        .expect("the numbers are seeded");
    assert_eq!(*report.outcome(), RunRecord::passed());
    assert_eq!(
        store.get::<Vec<Squared>>(),
        Some(vec![
            Squared { value: 1 },
            Squared { value: 4 },
            Squared { value: 9 }
        ])
    );
    assert_eq!(store.get::<Total>().map(|total| total.value), Some(14));
    let square = report
        .invocation("square")
        .expect("the fanned task is recorded");
    assert_eq!(
        square
            .attempts
            .iter()
            .map(|attempt| (attempt.id().attempt, attempt.outcome().clone()))
            .collect::<Vec<_>>(),
        [(0, RunRecord::passed())],
        "the task's own record is its items' gathering"
    );
    let paths: Vec<&str> = square
        .instances
        .iter()
        .map(|instance| instance.id().path.as_str())
        .collect();
    assert_eq!(
        paths,
        ["gate/square@1", "gate/square@2", "gate/square@3"],
        "an item goes by its key"
    );
    assert!(
        square.instances.iter().all(|instance| instance
            .attempts
            .last()
            .is_some_and(|attempt| attempt.process_record.is_some())),
        "each item ran as a process of its own"
    );
    let second = TaskAttempt::read(
        &report
            .dir()
            .join("gate")
            .join("square@2#1")
            .join(RECORD_FILE),
    )
    .expect("an item keeps its record in a folder named by its key");
    assert_eq!(second.evidence, ["squared 2"]);
    assert_eq!(
        outcome_of(&report, "unsummed"),
        Some(RunRecord::unneeded("gate/square".to_string()))
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn an_item_that_fails_fails_its_fanned_task_naming_it_and_the_failure_s_readers_answer() {
    let root = root("fanned-failure");
    let (report, store) = runner()
        .run(&squares("square"), numbered(&[1, -2, 3]), &root)
        .expect("the numbers are seeded");
    let reason = "1 of 3 items did not pass: gate/square@-2".to_string();
    assert_eq!(
        outcome_of(&report, "square"),
        Some(RunRecord::failed(reason.clone()))
    );
    let item = report
        .invocation("square")
        .and_then(|square| square.instances[1].attempts.last())
        .map(|attempt| attempt.outcome().clone());
    assert_eq!(
        item,
        Some(RunRecord::failed("-2 has no square here".to_string()))
    );
    assert_eq!(
        outcome_of(&report, "sum"),
        Some(RunRecord::skipped("gate/square".to_string()))
    );
    assert_eq!(
        store.get::<Unsummed>().map(|unsummed| unsummed.reason),
        Some(reason)
    );
    assert!(matches!(report.outcome().ended, Some(Ended::Failed(_))));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn the_items_of_a_fanned_task_run_at_the_same_time() {
    let root = root("fanned-parallel");
    let (report, _) = runner()
        .slots(4)
        .run(&squares("slow_square"), numbered(&[1, 2, 3]), &root)
        .expect("the numbers are seeded");
    assert_eq!(*report.outcome(), RunRecord::passed());
    let began: Vec<_> = report
        .invocation("slow_square")
        .expect("the fanned task is recorded")
        .instances
        .iter()
        .filter_map(|instance| instance.attempts.last().and_then(|attempt| attempt.began()))
        .collect();
    assert_eq!(began.len(), 3);
    let spread = began
        .iter()
        .map(|&at| seconds_between(at, began[0]))
        .fold(0.0, f64::max);
    assert!(
        spread < 0.15,
        "three 300ms items began {spread}s apart: they ran one after the other"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn each_item_of_a_cached_fanned_task_is_replayed_on_its_own() {
    let root = root("fanned-cached");
    let plan = squares("square");
    runner()
        .run(&plan, numbered(&[1, 2, 3]), &root)
        .expect("the numbers are seeded");
    let (report, store) = runner()
        .run(&plan, numbered(&[1, 5, 3]), &root)
        .expect("the numbers are seeded");
    let replayed: Vec<bool> = report
        .invocation("square")
        .expect("the fanned task is recorded")
        .instances
        .iter()
        .map(|instance| {
            instance
                .attempts
                .last()
                .is_some_and(|attempt| attempt.replayed)
        })
        .collect();
    assert_eq!(replayed, [true, false, true], "only the changed item ran");
    assert_eq!(store.get::<Total>().map(|total| total.value), Some(35));
    assert!(
        report
            .dir()
            .join("gate")
            .join("square@1#0")
            .join(RECORD_FILE)
            .is_file(),
        "a replayed item keeps its record in its own folder"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn only_an_output_its_cache_test_accepts_is_replayed_and_any_other_is_computed_again() {
    let root = root("cache-tested");
    let plan = squares("square_cached_when_even");
    let replayed = |report: &RunReport| -> Vec<(bool, bool)> {
        report
            .invocation("square_cached_when_even")
            .expect("the fanned task is recorded")
            .instances
            .iter()
            .filter_map(|instance| instance.attempts.last())
            .map(|attempt| (attempt.replayed, attempt.cache_key.is_some()))
            .collect()
    };
    let (first, _) = runner()
        .run(&plan, numbered(&[1, 2, 3]), &root)
        .expect("the numbers are seeded");
    assert_eq!(
        replayed(&first),
        [(false, false), (false, true), (false, false)],
        "only the even square is named in the cache"
    );
    let (second, store) = runner()
        .run(&plan, numbered(&[1, 2, 3]), &root)
        .expect("the numbers are seeded");
    assert_eq!(
        replayed(&second),
        [(false, false), (true, true), (false, false)],
        "the odd squares are computed again on the same inputs"
    );
    assert_eq!(store.get::<Total>().map(|total| total.value), Some(14));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_task_reading_a_failure_is_unneeded_when_that_task_passes_and_the_run_stays_clean() {
    let root = root("unneeded");
    let (report, store) = runner()
        .run(&repair(), sourced(true), &root)
        .expect("the source is seeded");
    assert_eq!(*report.outcome(), RunRecord::passed());
    assert_eq!(outcome_of(&report, "commit"), Some(RunRecord::passed()));
    assert_eq!(
        outcome_of(&report, "revert"),
        Some(RunRecord::unneeded("gate/check".to_string()))
    );
    assert!(
        store.get::<Reverted>().is_none(),
        "a branch not taken leaves no value"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_task_reading_a_failure_runs_when_that_task_fails_and_the_passed_branch_is_skipped() {
    let root = root("reverted");
    let (report, store) = runner()
        .run(&repair(), sourced(false), &root)
        .expect("the source is seeded");
    assert!(matches!(report.outcome().ended, Some(Ended::Failed(_))));
    assert_eq!(
        outcome_of(&report, "check"),
        Some(RunRecord::failed(
            "lib.rs fixed does not compile".to_string()
        ))
    );
    assert_eq!(
        outcome_of(&report, "commit"),
        Some(RunRecord::skipped("gate/check".to_string()))
    );
    assert_eq!(outcome_of(&report, "revert"), Some(RunRecord::passed()));
    let revert = report.attempt("revert").expect("the revert task ran");
    assert_eq!(
        revert.evidence,
        ["undid lib.rs fixed as gate/check: lib.rs fixed does not compile"]
    );
    assert_eq!(
        store.get::<Reverted>().map(|reverted| reverted.text),
        Some("lib.rs fixed".to_string())
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn what_stands_on_a_branch_not_taken_is_unneeded_for_the_same_reason() {
    let root = root("branch");
    let (report, _) = runner()
        .run(&repair(), sourced(true), &root)
        .expect("the source is seeded");
    assert_eq!(*report.outcome(), RunRecord::passed());
    assert_eq!(
        outcome_of(&report, "archive"),
        Some(RunRecord::unneeded("gate/check".to_string()))
    );
    let (report, _) = runner()
        .run(&repair(), sourced(false), &root)
        .expect("the source is seeded");
    assert_eq!(outcome_of(&report, "archive"), Some(RunRecord::passed()));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_task_reading_a_completion_runs_whatever_happened_to_its_producer() {
    let root = root("completion");
    let (_, store) = runner()
        .run(&repair(), sourced(true), &root)
        .expect("the source is seeded");
    assert_eq!(
        store.get::<Told>().map(|told| told.line),
        Some("passed unneeded by gate/check".to_string())
    );
    let (_, store) = runner()
        .run(&repair(), sourced(false), &root)
        .expect("the source is seeded");
    assert_eq!(
        store.get::<Told>().map(|told| told.line),
        Some("skipped by gate/check passed".to_string())
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_cached_task_is_replayed_from_the_store_on_the_same_inputs_and_computed_on_others() {
    let root = root("cached");
    let plan = Plan::new("gate", &listed(), &["costly"]).expect("valid");
    let (first, first_store) = runner()
        .run(&plan, seeded("repo", true), &root)
        .expect("the root is seeded");
    let computed = first_store.get::<Costly>().expect("computed");
    let attempt = first.attempt("costly").expect("ran");
    assert!(!attempt.replayed && attempt.id().attempt == 1 && attempt.process_record.is_some());
    let (second, second_store) = runner()
        .run(&plan, seeded("repo", true), &root)
        .expect("the root is seeded");
    let replayed = second.attempt("costly").expect("recorded");
    assert!(replayed.replayed, "the same inputs replay the output");
    assert_eq!(replayed.id().attempt, 0, "no process ran");
    assert!(replayed.process_record.is_none());
    assert_eq!(second_store.get::<Costly>(), Some(computed));
    assert_eq!(
        (&replayed.output, &replayed.cache_key),
        (&attempt.output, &attempt.cache_key),
        "a replay names the output and the key the first run left"
    );
    assert!(attempt.cache_key.is_some(), "a cached task names its key");
    let hash = attempt
        .output_hash()
        .expect("a passed task names its output");
    let kept = fs::read(root.join(OUTPUTS_DIR).join(hash.as_str())).expect("the output is kept");
    assert_eq!(
        TaskOutputHash::of(&kept),
        hash,
        "the output is kept under the hash of its bytes"
    );
    let written = TaskAttempt::read(&second.dir().join("gate").join("costly#0").join(RECORD_FILE))
        .expect("a replay leaves its record");
    assert_eq!(&written, replayed, "a replay's record names what it took");
    let (third, third_store) = runner()
        .run(&plan, seeded("other", true), &root)
        .expect("the root is seeded");
    assert!(!third.attempt("costly").expect("ran").replayed);
    assert_ne!(third_store.get::<Costly>(), second_store.get::<Costly>());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_cold_run_computes_a_cached_task_again_and_still_names_what_it_left_in_the_cache() {
    let root = root("cold");
    let plan = Plan::new("gate", &listed(), &["costly"]).expect("valid");
    runner()
        .run(&plan, seeded("repo", true), &root)
        .expect("the root is seeded");
    let (cold, cold_store) = runner()
        .cold()
        .run(&plan, seeded("repo", true), &root)
        .expect("the root is seeded");
    let computed = cold.attempt("costly").expect("ran");
    assert!(
        !computed.replayed && computed.process_record.is_some(),
        "the same inputs are computed again"
    );
    assert!(computed.cache_key.is_some(), "and what they left is cached");
    let (warm, warm_store) = runner()
        .run(&plan, seeded("repo", true), &root)
        .expect("the root is seeded");
    assert!(warm.attempt("costly").expect("recorded").replayed);
    assert_eq!(
        warm_store.get::<Costly>(),
        cold_store.get::<Costly>(),
        "the next run replays what the cold one computed"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn the_task_executable_is_handed_the_words_given_before_each_attempt_s_folder() {
    let root = root("worded");
    let plan = Plan::new("gate", &listed(), &["scan"]).expect("valid");
    // A program refusing any call that does not begin with the words, then
    // performing the attempt as the test tasks do.
    let executable = root.join("worded");
    fs::write(
        &executable,
        format!(
            "#!/bin/sh\n[ \"$1\" = attempt ] || exit 9\nshift\nexec {} \"$@\"\n",
            env!("CARGO_BIN_EXE_test-tasks")
        ),
    )
    .expect("a file is written");
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).expect("made executable");
    let (report, _) = Runner::new()
        .task_executable(&executable)
        .task_arguments(&["attempt"])
        .run(&plan, seeded("repo", true), &root)
        .expect("the run is verified");
    assert_eq!(*report.outcome(), RunRecord::passed());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_run_leaves_each_attempt_s_order_record_and_printing_on_disk_and_its_output_in_the_store() {
    let root = root("disk");
    let (report, _) = runner()
        .run(&gate(), seeded("repo", true), &root)
        .expect("the root is seeded");
    assert_eq!(
        report.dir(),
        root.join(RUNS_DIR).join(report.run().to_string())
    );
    for name in ["scan", "resolve", "cycles", "boundaries", "tools", "ledger"] {
        let attempt = report.dir().join("gate").join(format!("{name}#1"));
        for file in [RECORD_FILE, STDOUT_FILE, STDERR_FILE] {
            assert!(attempt.join(file).is_file(), "{name} left {file}");
        }
        let recorded = TaskAttempt::read(&attempt.join(RECORD_FILE)).expect("readable");
        assert_eq!(
            Some(&recorded),
            report.attempt(name),
            "{name}'s folder keeps its record as the run completed it"
        );
    }
    assert!(
        report.dir().join(RUN_FILE).is_file(),
        "the run wrote its report"
    );
    assert!(
        !report.dir().join(RUNNING_FILE).exists(),
        "a finished run is no longer marked as going"
    );
    let written = RunReport::read(&report.dir().join(RUN_FILE)).expect("readable");
    assert_eq!(written, report, "the run's file holds its report whole");
    assert_eq!(written.invocations.len(), 6);
    let ledger = report.attempt("ledger").expect("the ledger ran");
    let hash = ledger.output.clone().expect("the ledger left an output");
    let output = fs::read(root.join(OUTPUTS_DIR).join(hash.as_str())).expect("the output is kept");
    assert_eq!(
        serde_json::from_slice::<Summary>(&output)
            .map(|summary| summary.line)
            .ok(),
        Some("41 42 clippy+test".to_string())
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_run_keeps_only_the_newest_runs_and_the_outputs_and_cache_entries_they_name() {
    let root = root("keep");
    let plan = Plan::new("gate", &listed(), &["costly"]).expect("valid");
    let runner = runner().keep(2);
    let (first, _) = runner
        .run(&plan, seeded("alpha", true), &root)
        .expect("seeded");
    runner
        .run(&plan, seeded("beta", true), &root)
        .expect("seeded");
    let (third, _) = runner
        .run(&plan, seeded("beta", true), &root)
        .expect("seeded");
    assert!(third.attempt("costly").expect("recorded").replayed);
    let runs: Vec<String> = fs::read_dir(root.join(RUNS_DIR))
        .expect("runs are listed")
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    assert_eq!(runs.len(), 2, "two runs are kept: {runs:?}");
    assert!(
        !runs.contains(&first.run().to_string()),
        "the oldest run is gone"
    );
    let outputs = fs::read_dir(root.join(OUTPUTS_DIR))
        .expect("the outputs are listed")
        .count();
    assert_eq!(outputs, 1, "only the output the kept runs name remains");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_chatty_task_process_s_output_is_capped_on_disk_and_counted_whole() {
    let root = root("cap");
    let plan = Plan::new("gate", &listed(), &["printer"]).expect("valid");
    let mut seeds = RunStore::new();
    seeds.put(&Chatter { bytes: 1 << 20 }).expect("encodes");
    let (report, _) = runner()
        .cap(1024)
        .run(&plan, seeds, &root)
        .expect("the chatter is seeded");
    assert_eq!(*report.outcome(), RunRecord::passed());
    let process_record = report
        .attempt("printer")
        .and_then(|attempt| attempt.process_record.as_ref())
        .expect("accounted");
    assert_eq!(
        (process_record.stdout_bytes, process_record.stdout_kept),
        (1 << 20, 1024)
    );
    let kept = fs::metadata(
        report
            .dir()
            .join("gate")
            .join("printer#1")
            .join(STDOUT_FILE),
    )
    .expect("the output file is there")
    .len();
    assert_eq!(kept, 1024);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_run_refuses_a_seed_of_a_type_a_task_produces() {
    let root = root("shadowed");
    let plan = Plan::new("gate", &listed(), &["scan", "resolve"]).expect("the plan is valid");
    let mut seeds = seeded("repo", true);
    seeds
        .put(&Repo {
            files: 1,
            parses: true,
        })
        .expect("a repo encodes");
    let refused = runner()
        .run(&plan, seeds, &root)
        .err()
        .expect("the seed shadows what scan produces");
    assert_eq!(
        refused.reasons,
        vec![RefusalReason::Shadowed {
            value_type: ValueType::of::<Repo>(),
            by: "scan".to_string()
        }]
    );
    assert_eq!(
        fs::read_dir(root.join(RUNS_DIR))
            .map(|entries| entries.count())
            .unwrap_or(0),
        0,
        "no run began"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_run_refuses_a_task_executable_that_is_not_an_executable_file() {
    let root = root("noexecutable");
    let plan = Plan::new("gate", &listed(), &["scan"]).expect("the plan is valid");
    let missing = PathBuf::from("/nonexistent/tasks");
    let refused = Runner::new()
        .task_executable(missing.clone())
        .run(&plan, seeded("repo", true), &root)
        .err()
        .expect("there is no task executable to run");
    assert!(
        matches!(&refused.reasons[..], [RefusalReason::NoTaskExecutable { path, .. }] if path == &missing),
        "{refused}"
    );
    assert_eq!(
        fs::read_dir(root.join(RUNS_DIR))
            .map(|entries| entries.count())
            .unwrap_or(0),
        0,
        "no run began"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_task_executable_that_cannot_start_fails_its_attempt() {
    let root = root("notaprogram");
    let plan = Plan::new("gate", &listed(), &["scan"]).expect("valid");
    // An executable file whose interpreter does not exist: it passes
    // verification and the kernel refuses to start it.
    let executable = root.join("executable");
    fs::write(&executable, "#!/nonexistent/interpreter\n").expect("a file is written");
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).expect("made executable");
    let (report, _) = Runner::new()
        .task_executable(executable)
        .run(&plan, seeded("repo", true), &root)
        .expect("the run is verified");
    let scan = report.attempt("scan").expect("recorded");
    match &scan.outcome().ended {
        Some(Ended::Failed(reason)) => {
            assert!(
                reason.starts_with("the task process could not start"),
                "{reason}"
            )
        }
        other => panic!("a missing task executable did not fail the attempt: {other:?}"),
    }
    assert_eq!(scan.id().attempt, 1);
    assert!(scan.process_record.is_none(), "no process ran");
    let _ = fs::remove_dir_all(&root);
}
