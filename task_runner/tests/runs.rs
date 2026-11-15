use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use task_runner::exhibit::products::Answered;
use task_runner::exhibit::products::Appetite;
use task_runner::exhibit::products::Chatter;
use task_runner::exhibit::products::Costly;
use task_runner::exhibit::products::Flaked;
use task_runner::exhibit::products::Flakiness;
use task_runner::exhibit::products::Graph;
use task_runner::exhibit::products::Held;
use task_runner::exhibit::products::Licenses;
use task_runner::exhibit::products::Repo;
use task_runner::exhibit::products::Reverted;
use task_runner::exhibit::products::Root;
use task_runner::exhibit::products::Said;
use task_runner::exhibit::products::Source;
use task_runner::exhibit::products::Summary;
use task_runner::exhibit::products::Told;
use task_runner::exhibit::products::Turns;
use task_runner::exhibit::products::Verdict;
use task_runner::job::cancel::Cancel;
use task_runner::job::id::RunId;
use task_runner::layout::KEY_FILE;
use task_runner::layout::ORDER_FILE;
use task_runner::layout::PRODUCT_FILE;
use task_runner::layout::RECORD_FILE;
use task_runner::layout::RUNNING_FILE;
use task_runner::layout::RUNS_DIR;
use task_runner::layout::RUN_FILE;
use task_runner::layout::SEEDS_DIR;
use task_runner::layout::STDERR_FILE;
use task_runner::layout::STDOUT_FILE;
use task_runner::layout::STORE_DIR;
use task_runner::plan::Plan;
use task_runner::registry::known;
use task_runner::run::observer::Event;
use task_runner::run::observer::Observer;
use task_runner::run::observer::Quiet;
use task_runner::run::refusal::Problem;
use task_runner::run::report::Report;
use task_runner::run::runner::Runner;
use task_runner::store::Need;
use task_runner::store::Store;
use task_runner::trace::outcome::Outcome;
use task_runner::worker::Order;

fn runner() -> Runner {
    Runner::new().worker(PathBuf::from(env!("CARGO_BIN_EXE_exhibit-worker")))
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
        &known(),
        &["scan", "resolve", "cycles", "boundaries", "tools", "ledger"],
    )
    .expect("the gate exhibit is a valid plan")
}

fn repair() -> Plan {
    Plan::new(
        "gate",
        &known(),
        &["fix", "check", "commit", "revert", "archive", "report"],
    )
    .expect("the repair exhibit is a valid plan")
}

fn seeded(name: &str, parses: bool) -> Store {
    let mut store = Store::new();
    store
        .put(&Root {
            name: name.to_string(),
            parses,
        })
        .expect("a root encodes");
    store
}

fn sourced(compiles: bool) -> Store {
    let mut store = Store::new();
    store
        .put(&Source {
            name: "lib.rs".to_string(),
            compiles,
        })
        .expect("a source encodes");
    store
}

fn outcome_of(report: &Report, name: &str) -> Option<Outcome> {
    report.step(name).map(|step| step.outcome.clone())
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
        &known(),
        &["ledger", "tools", "boundaries", "cycles", "resolve", "scan"],
    )
    .expect("the plan is valid");
    let report = runner()
        .run(&plan, seeded("repo", true), &root)
        .expect("the root is seeded");
    assert_eq!(report.outcome, Outcome::Ok);
    assert_eq!(
        report.get::<Summary>().map(|summary| summary.line),
        Some("41 42 clippy+test".to_string())
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn independent_tasks_run_at_the_same_time() {
    let root = root("parallel");
    let report = runner()
        .run(&gate(), seeded("repo", true), &root)
        .expect("the root is seeded");
    assert_eq!(report.outcome, Outcome::Ok);
    let cycles = report
        .step("cycles")
        .and_then(|step| step.began)
        .expect("began");
    let bounds = report
        .step("boundaries")
        .and_then(|step| step.began)
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
    let report = runner()
        .slots(1)
        .run(&gate(), seeded("repo", true), &root)
        .expect("the root is seeded");
    assert_eq!(report.outcome, Outcome::Ok);
    let cycles = report
        .step("cycles")
        .and_then(|step| step.began)
        .expect("began");
    let bounds = report
        .step("boundaries")
        .and_then(|step| step.began)
        .expect("began");
    assert!(
        seconds_between(cycles, bounds) >= 0.2,
        "two 200ms tasks on one slot began {}s apart: they overlapped",
        seconds_between(cycles, bounds)
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn every_task_keeps_its_own_step_in_plan_order_with_path_ids() {
    let root = root("ids");
    let report = runner()
        .run(&gate(), seeded("repo", true), &root)
        .expect("the root is seeded");
    let ids: Vec<&str> = report
        .steps()
        .iter()
        .map(|step| step.job.path.as_str())
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
    let tools = report.step("tools").expect("the tools task ran");
    let inner: Vec<&str> = tools
        .children
        .iter()
        .map(|step| step.job.path.as_str())
        .collect();
    assert_eq!(inner, ["gate/tools/clippy", "gate/tools/test"]);
    let scan = report.step("scan").expect("the scan task ran");
    assert_eq!(scan.evidence, ["scanned repo"]);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_run_has_its_own_id_and_every_step_carries_it_with_the_attempt() {
    let root = root("runs");
    let one = runner()
        .run(&gate(), seeded("repo", true), &root)
        .expect("the root is seeded");
    let two = runner()
        .run(&gate(), seeded("repo", true), &root)
        .expect("the root is seeded");
    assert_ne!(one.run, two.run);
    assert!(one.steps().iter().all(|step| step.job.run == one.run));
    assert!(two.steps().iter().all(|step| step.job.run == two.run));
    let scan = one.step("scan").expect("the scan task ran");
    assert_eq!(scan.job.to_string(), format!("{}/gate/scan#1", one.run));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn an_attempt_records_its_slot_and_the_time_it_waited_for_one() {
    let root = root("slots");
    let plan = Plan::new("gate", &known(), &["cycles", "boundaries"]).expect("valid");
    let mut seeds = Store::new();
    seeds.put(&Graph { nodes: 4 }).expect("encodes");
    let report = runner()
        .slots(1)
        .run(&plan, seeds, &root)
        .expect("the graph is seeded");
    let first = report.step("cycles").expect("ran");
    let second = report.step("boundaries").expect("ran");
    assert_eq!((first.slot, second.slot), (Some(0), Some(0)));
    let first_wait = first.wait.expect("an attempt records its wait");
    let second_wait = second.wait.expect("an attempt records its wait");
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
    let plan = Plan::new("gate", &known(), &["hungry"]).expect("valid");
    let mut seeds = Store::new();
    seeds.put(&Appetite { mebibytes: 64 }).expect("encodes");
    let report = runner()
        .run(&plan, seeds, &root)
        .expect("the appetite is seeded");
    assert_eq!(report.outcome, Outcome::Ok);
    assert_eq!(report.get::<Held>().map(|held| held.bytes), Some(64 << 20));
    let step = report.step("hungry").expect("ran");
    let account = step.account.as_ref().expect("an attempt has an account");
    assert_eq!(
        (account.exit_code, account.signal, account.killed),
        (Some(0), None, false)
    );
    assert!(
        account.usage.max_rss_bytes >= 64 << 20,
        "a process holding 64 MiB peaked at {} bytes",
        account.usage.max_rss_bytes
    );
    assert!(account.usage.cpu() > Duration::ZERO);
    assert!(
        account.usage.minor_faults > 0,
        "touching 64 MiB faults pages in"
    );
    let startup = account
        .startup
        .expect("the worker marked when its body began");
    assert!(
        startup < account.wall,
        "startup {startup:?} within a wall of {:?}",
        account.wall
    );
    assert!(
        step.resources.cpu.is_some(),
        "the frame measured its own thread too"
    );
    assert!(account.wall >= step.resources.wall);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_report_sums_the_kernel_s_account_of_every_attempt() {
    let root = root("bill");
    let plan = Plan::new("gate", &known(), &["flaky"]).expect("valid");
    let mut seeds = Store::new();
    seeds.put(&Flakiness { passes_at: 2 }).expect("encodes");
    let report = runner()
        .run(&plan, seeds, &root)
        .expect("the flakiness is seeded");
    assert_eq!(report.outcome, Outcome::Ok);
    let attempts = report.attempts_of("flaky");
    assert_eq!(attempts.len(), 2);
    let billed: Duration = attempts
        .iter()
        .filter_map(|step| step.account.as_ref())
        .map(|account| account.usage.cpu())
        .sum();
    assert_eq!(report.usage.cpu(), billed);
    assert!(report.usage.cpu() > Duration::ZERO);
    let peak = attempts
        .iter()
        .filter_map(|step| step.account.as_ref())
        .map(|account| account.usage.max_rss_bytes)
        .max()
        .expect("accounted");
    assert_eq!(report.usage.max_rss_bytes, peak);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_failed_task_skips_every_task_standing_on_it_and_the_rest_still_run() {
    let root = root("skip");
    let plan = Plan::new(
        "gate",
        &known(),
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
    let report = runner()
        .run(&plan, seeded("broken", false), &root)
        .expect("the root is seeded");
    assert!(matches!(report.outcome, Outcome::Failed(_)));
    assert_eq!(outcome_of(&report, "scan"), Some(Outcome::Ok));
    assert_eq!(
        outcome_of(&report, "resolve"),
        Some(Outcome::Failed("the parser choked".to_string()))
    );
    assert_eq!(
        outcome_of(&report, "cycles"),
        Some(Outcome::Skipped("gate/resolve".to_string()))
    );
    assert_eq!(
        outcome_of(&report, "ledger"),
        Some(Outcome::Skipped("gate/resolve".to_string()))
    );
    assert_eq!(outcome_of(&report, "licenses"), Some(Outcome::Ok));
    assert_eq!(report.get::<Licenses>().map(|found| found.bytes), Some(6));
    let cycles = report
        .step("cycles")
        .expect("the skipped task has a record");
    assert_eq!(cycles.job.attempt, 0, "a skipped task never attempted");
    assert!(cycles.began.is_none() && cycles.account.is_none());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_body_that_panics_fails_its_attempt_and_the_rest_of_the_run_goes_on() {
    let root = root("panic");
    let plan = Plan::new("gate", &known(), &["scan", "boom"]).expect("the plan is valid");
    let report = runner()
        .run(&plan, seeded("repo", true), &root)
        .expect("the root is seeded");
    assert!(matches!(report.outcome, Outcome::Failed(_)));
    assert_eq!(outcome_of(&report, "scan"), Some(Outcome::Ok));
    let boom = report.step("boom").expect("recorded");
    match &boom.outcome {
        Outcome::Failed(reason) => {
            assert!(reason.contains("exited with code 101"), "{reason}");
            assert!(reason.contains("boom"), "{reason}");
        }
        other => panic!("a panic did not fail the attempt: {other:?}"),
    }
    let account = boom.account.as_ref().expect("accounted");
    assert_eq!(account.exit_code, Some(101));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_failed_attempt_is_tried_again_up_to_the_retries_and_each_keeps_its_record() {
    let root = root("retries");
    let plan = Plan::new("gate", &known(), &["flaky"]).expect("the plan is valid");
    let mut seeds = Store::new();
    seeds.put(&Flakiness { passes_at: 3 }).expect("encodes");
    let report = runner()
        .run(&plan, seeds, &root)
        .expect("the flakiness is seeded");
    assert_eq!(report.outcome, Outcome::Ok);
    assert_eq!(report.get::<Flaked>().map(|flaked| flaked.attempt), Some(3));
    let attempts: Vec<(u32, Outcome)> = report
        .attempts_of("flaky")
        .iter()
        .map(|step| (step.job.attempt, step.outcome.clone()))
        .collect();
    assert_eq!(
        attempts,
        [
            (1, Outcome::Failed("attempt 1 failed".to_string())),
            (2, Outcome::Failed("attempt 2 failed".to_string())),
            (3, Outcome::Ok)
        ]
    );
    for attempt in 1..=3 {
        assert!(
            report
                .dir
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
    let plan = Plan::new("gate", &known(), &["never", "answer"]).expect("the plan is valid");
    let report = runner()
        .run(&plan, seeded("repo", true), &root)
        .expect("the root is seeded");
    assert_eq!(report.attempts_of("never").len(), 2);
    assert_eq!(report.attempts_of("answer").len(), 1);
    assert_eq!(
        report.get::<Answered>().map(|answered| answered.line),
        Some("gate/never#2".to_string())
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_task_out_of_time_is_timed_out_and_its_process_tree_ended_so_the_slot_is_freed() {
    let root = root("timeout");
    let plan = Plan::new("gate", &known(), &["sleeper", "quick"]).expect("the plan is valid");
    let report = runner()
        .slots(1)
        .run(&plan, seeded("repo", true), &root)
        .expect("the root is seeded");
    assert!(
        report.seconds < 3.0,
        "the run took {}s: the sleeping command held the slot",
        report.seconds
    );
    assert_eq!(outcome_of(&report, "sleeper"), Some(Outcome::TimedOut));
    assert_eq!(outcome_of(&report, "quick"), Some(Outcome::Ok));
    assert!(matches!(report.outcome, Outcome::Failed(_)));
    let sleeper = report.step("sleeper").expect("recorded");
    let account = sleeper.account.as_ref().expect("accounted");
    assert!(account.killed, "the run stopped it");
    assert!(
        account.wall < Duration::from_secs(2),
        "the worker lived {:?} past a 100ms allowance",
        account.wall
    );
    let command = sleeper
        .resources
        .children
        .first()
        .expect("the worker recorded the command it ran before it stopped");
    assert_eq!(
        command.exit_code, None,
        "the five second sleep did not end on its own"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_body_that_checks_in_stops_at_its_next_checkpoint_once_out_of_time() {
    let root = root("checkpoint");
    let plan = Plan::new("gate", &known(), &["looper", "quick"]).expect("the plan is valid");
    let report = runner()
        .slots(1)
        .run(&plan, seeded("repo", true), &root)
        .expect("the root is seeded");
    assert!(report.seconds < 2.0, "the run took {}s", report.seconds);
    assert_eq!(outcome_of(&report, "looper"), Some(Outcome::TimedOut));
    assert_eq!(outcome_of(&report, "quick"), Some(Outcome::Ok));
    assert!(report.get::<Turns>().is_none());
    let looper = report.step("looper").expect("recorded");
    assert!(
        looper.evidence.first().is_some_and(|line| line == "turn 0"),
        "the record the body left on stopping is kept: {:?}",
        looper.evidence
    );
    let account = looper.account.as_ref().expect("accounted");
    assert_eq!(
        (account.signal, account.exit_code),
        (None, Some(1)),
        "a body that stopped at its checkpoint ended on its own"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_body_deaf_to_the_request_to_stop_is_killed_once_the_grace_is_spent() {
    let root = root("deaf");
    let plan = Plan::new("gate", &known(), &["deaf"]).expect("the plan is valid");
    let report = runner()
        .grace(Duration::from_millis(100))
        .run(&plan, seeded("repo", true), &root)
        .expect("the root is seeded");
    assert!(report.seconds < 2.0, "the run took {}s", report.seconds);
    assert_eq!(outcome_of(&report, "deaf"), Some(Outcome::TimedOut));
    let deaf = report.step("deaf").expect("recorded");
    let account = deaf.account.as_ref().expect("accounted");
    assert_eq!(account.signal, Some(9), "it was killed");
    assert!(account.killed);
    assert!(
        deaf.began.is_some(),
        "the run knows when it began even without a record"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn cancelling_a_run_stops_what_is_out_and_about_to_go_out_while_a_failure_s_readers_still_answer() {
    let root = root("cancel");
    let plan = Plan::new(
        "gate",
        &known(),
        &["slow", "later", "after", "undo", "tell"],
    )
    .expect("valid");
    let cancel = Cancel::new();
    let token = cancel.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(300));
        token.cancel();
    });
    let report = runner()
        .slots(1)
        .run_with(&plan, seeded("repo", true), &root, &cancel, &mut Quiet)
        .expect("the root is seeded");
    assert!(report.seconds < 2.5, "the run took {}s", report.seconds);
    assert_eq!(outcome_of(&report, "slow"), Some(Outcome::Cancelled));
    assert_eq!(outcome_of(&report, "later"), Some(Outcome::Cancelled));
    assert_eq!(
        report.step("later").map(|step| step.job.attempt),
        Some(0),
        "a task about to go out never attempted"
    );
    assert_eq!(
        outcome_of(&report, "after"),
        Some(Outcome::Skipped("gate/slow".to_string()))
    );
    assert_eq!(outcome_of(&report, "undo"), Some(Outcome::Ok));
    assert_eq!(
        report.get::<Said>().map(|said| said.line),
        Some("Cancelled Ok".to_string())
    );
    assert!(matches!(report.outcome, Outcome::Failed(_)));
    let _ = fs::remove_dir_all(&root);
}

// Writes down each event as one line, so the order can be read back.
struct Heard(Vec<String>);

impl Observer for Heard {
    fn event(&mut self, event: Event<'_>) {
        self.0.push(match event {
            Event::Started { job, slot } => format!("started {} on {slot}", job.path),
            Event::Ended { step } => format!("ended {} {:?}", step.job.path, step.outcome),
            Event::Settled { path, outcome } => format!("settled {path} {outcome:?}"),
            Event::Finished { report } => format!("finished {:?}", report.outcome),
        })
    }
}

#[test]
fn an_observer_hears_each_start_each_record_each_settling_and_the_finish_in_order() {
    let root = root("observed");
    let mut heard = Heard(Vec::new());
    let report = runner()
        .run_with(
            &gate(),
            seeded("repo", true),
            &root,
            &Cancel::new(),
            &mut heard,
        )
        .expect("the root is seeded");
    assert_eq!(report.outcome, Outcome::Ok);
    let heard = heard.0;
    assert_eq!(
        heard.first().map(String::as_str),
        Some("started gate/scan on 0")
    );
    assert_eq!(heard.last().map(String::as_str), Some("finished Ok"));
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
    let plan = Plan::new("gate", &known(), &["scan", "resolve"]).expect("the plan is valid");
    let refused = runner().run(&plan, Store::new(), &root);
    assert_eq!(
        refused.err().map(|refused| refused.problems),
        Some(vec![Problem::Unseeded {
            need: Need::of::<Root>()
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
fn a_plan_run_inside_a_task_nests_its_steps_under_that_task() {
    let root = root("nested");
    let plan = Plan::new("gate", &known(), &["scan", "resolve", "judge"]).expect("valid");
    let report = runner()
        .run(&plan, seeded("repo", true), &root)
        .expect("the root is seeded");
    assert_eq!(report.outcome, Outcome::Ok);
    assert_eq!(
        report.get::<Verdict>().map(|verdict| verdict.findings),
        Some(83)
    );
    let judge = report.step("judge").expect("the judge task ran");
    let ids: Vec<&str> = judge
        .children
        .iter()
        .map(|step| step.job.path.as_str())
        .collect();
    assert_eq!(
        ids,
        ["gate/judge/1/laws/cycles", "gate/judge/1/laws/boundaries"]
    );
    assert!(
        judge.children.iter().all(|step| step.job.run == report.run),
        "an inner plan runs in the same run"
    );
    assert!(
        judge
            .children
            .iter()
            .all(|step| step.account.is_some() && step.slot.is_some()),
        "each inner task was a process of its own"
    );
    let nested = report
        .dir
        .join("gate")
        .join("judge#1")
        .join(RUNS_DIR)
        .join("1")
        .join("laws")
        .join("cycles#1");
    assert!(
        nested.join(RECORD_FILE).is_file(),
        "an inner attempt's folder sits under its parent's"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_plan_run_inside_an_attempt_keys_its_products_by_the_identity_of_the_run_around_it() {
    let root = root("identity");
    // A worker whose own bytes differ from the binary it hands each attempt
    // to: a plan run inside an attempt that hashed the binary it runs in would
    // key its products by another identity than the run around it.
    let worker = root.join("worker");
    fs::write(
        &worker,
        format!(
            "#!/bin/sh\nexec '{}' \"$@\"\n",
            env!("CARGO_BIN_EXE_exhibit-worker")
        ),
    )
    .expect("a file is written");
    fs::set_permissions(&worker, fs::Permissions::from_mode(0o755)).expect("made executable");
    let plan = Plan::new("gate", &known(), &["scan", "resolve", "rejudge"]).expect("valid");
    let report = Runner::new()
        .worker(worker)
        .run(&plan, seeded("repo", true), &root)
        .expect("the root is seeded");
    assert_eq!(report.outcome, Outcome::Ok);
    let attempt = report.dir.join("gate").join("rejudge#1");
    let around = identity(&attempt);
    for number in ["1", "2"] {
        let inner = attempt
            .join(RUNS_DIR)
            .join(number)
            .join("laws")
            .join("cycles#1");
        assert_eq!(
            identity(&inner),
            around,
            "run {number} keys by the identity of the run around it"
        );
    }
    let _ = fs::remove_dir_all(&root);
}

// The identity the order of an attempt hands it.
fn identity(attempt: &Path) -> u64 {
    let bytes = fs::read(attempt.join(ORDER_FILE)).expect("the order is kept");
    serde_json::from_slice::<Order>(&bytes)
        .expect("the order decodes")
        .identity
}

#[test]
fn two_plans_run_inside_one_attempt_each_keep_their_own_folder_seeds_record_and_ids() {
    let root = root("twice");
    let plan = Plan::new("gate", &known(), &["scan", "resolve", "rejudge"]).expect("valid");
    let report = runner()
        .run(&plan, seeded("repo", true), &root)
        .expect("the root is seeded");
    assert_eq!(report.outcome, Outcome::Ok);
    assert_eq!(
        report.get::<Verdict>().map(|verdict| verdict.findings),
        Some(83 + 43),
        "each run read its own seeds"
    );
    let rejudge = report.step("rejudge").expect("the rejudge task ran");
    let mut ids: Vec<&str> = Vec::new();
    for step in &rejudge.children {
        ids.push(step.job.path.as_str());
        for inner in &step.children {
            ids.push(inner.job.path.as_str());
        }
    }
    assert_eq!(
        ids,
        [
            "gate/rejudge/1/laws/cycles",
            "gate/rejudge/1/laws/boundaries",
            "gate/rejudge/again",
            "gate/rejudge/again/2/laws/cycles",
            "gate/rejudge/again/2/laws/boundaries",
        ],
        "a run from a step counts on from the runs before it"
    );
    let attempt = report.dir.join("gate").join("rejudge#1");
    for number in ["1", "2"] {
        let run = attempt.join(RUNS_DIR).join(number);
        assert!(run.join(SEEDS_DIR).is_dir(), "run {number} keeps its seeds");
        assert!(
            run.join(RUN_FILE).is_file(),
            "run {number} keeps its record"
        );
        assert!(
            run.join("laws")
                .join("cycles#1")
                .join(RECORD_FILE)
                .is_file(),
            "run {number} keeps its attempts"
        );
    }
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_task_reading_a_failure_is_unneeded_when_that_task_passes_and_the_run_stays_clean() {
    let root = root("unneeded");
    let report = runner()
        .run(&repair(), sourced(true), &root)
        .expect("the source is seeded");
    assert_eq!(report.outcome, Outcome::Ok);
    assert_eq!(outcome_of(&report, "commit"), Some(Outcome::Ok));
    assert_eq!(
        outcome_of(&report, "revert"),
        Some(Outcome::Unneeded("gate/check".to_string()))
    );
    assert!(
        report.get::<Reverted>().is_none(),
        "a branch not taken leaves no value"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_task_reading_a_failure_runs_when_that_task_fails_and_the_passed_branch_is_skipped() {
    let root = root("reverted");
    let report = runner()
        .run(&repair(), sourced(false), &root)
        .expect("the source is seeded");
    assert!(matches!(report.outcome, Outcome::Failed(_)));
    assert_eq!(
        outcome_of(&report, "check"),
        Some(Outcome::Failed("lib.rs fixed does not compile".to_string()))
    );
    assert_eq!(
        outcome_of(&report, "commit"),
        Some(Outcome::Skipped("gate/check".to_string()))
    );
    assert_eq!(outcome_of(&report, "revert"), Some(Outcome::Ok));
    let revert = report.step("revert").expect("the revert task ran");
    assert_eq!(
        revert.evidence,
        ["undid lib.rs fixed as gate/check: lib.rs fixed does not compile"]
    );
    assert_eq!(
        report.get::<Reverted>().map(|reverted| reverted.text),
        Some("lib.rs fixed".to_string())
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn what_stands_on_a_branch_not_taken_is_unneeded_for_the_same_reason() {
    let root = root("branch");
    let report = runner()
        .run(&repair(), sourced(true), &root)
        .expect("the source is seeded");
    assert_eq!(report.outcome, Outcome::Ok);
    assert_eq!(
        outcome_of(&report, "archive"),
        Some(Outcome::Unneeded("gate/check".to_string()))
    );
    let report = runner()
        .run(&repair(), sourced(false), &root)
        .expect("the source is seeded");
    assert_eq!(outcome_of(&report, "archive"), Some(Outcome::Ok));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_task_reading_a_completion_runs_whatever_happened_to_its_producer() {
    let root = root("completion");
    let report = runner()
        .run(&repair(), sourced(true), &root)
        .expect("the source is seeded");
    assert_eq!(
        report.get::<Told>().map(|told| told.line),
        Some("Ok Unneeded(\"gate/check\")".to_string())
    );
    let report = runner()
        .run(&repair(), sourced(false), &root)
        .expect("the source is seeded");
    assert_eq!(
        report.get::<Told>().map(|told| told.line),
        Some("Skipped(\"gate/check\") Ok".to_string())
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_cached_task_is_replayed_from_the_store_on_the_same_inputs_and_computed_on_others() {
    let root = root("cached");
    let plan = Plan::new("gate", &known(), &["costly"]).expect("valid");
    let first = runner()
        .run(&plan, seeded("repo", true), &root)
        .expect("the root is seeded");
    let computed = first.get::<Costly>().expect("computed");
    let step = first.step("costly").expect("ran");
    assert!(!step.replayed && step.job.attempt == 1 && step.account.is_some());
    let second = runner()
        .run(&plan, seeded("repo", true), &root)
        .expect("the root is seeded");
    let replayed = second.step("costly").expect("recorded");
    assert!(replayed.replayed, "the same inputs replay the product");
    assert_eq!(replayed.job.attempt, 0, "no process ran");
    assert!(replayed.account.is_none());
    assert_eq!(second.get::<Costly>(), Some(computed));
    let key = second.dir.join("gate").join("costly#0").join(KEY_FILE);
    assert!(key.is_file(), "a replayed attempt records the key it took");
    let third = runner()
        .run(&plan, seeded("other", true), &root)
        .expect("the root is seeded");
    assert!(!third.step("costly").expect("ran").replayed);
    assert_ne!(third.get::<Costly>(), second.get::<Costly>());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_run_leaves_every_attempt_s_order_record_product_and_output_on_disk() {
    let root = root("disk");
    let report = runner()
        .run(&gate(), seeded("repo", true), &root)
        .expect("the root is seeded");
    assert_eq!(report.dir, root.join(RUNS_DIR).join(report.run.to_string()));
    for name in ["scan", "resolve", "cycles", "boundaries", "tools", "ledger"] {
        let attempt = report.dir.join("gate").join(format!("{name}#1"));
        for file in [
            ORDER_FILE,
            RECORD_FILE,
            PRODUCT_FILE,
            STDOUT_FILE,
            STDERR_FILE,
        ] {
            assert!(attempt.join(file).is_file(), "{name} left {file}");
        }
    }
    assert!(
        report.dir.join(RUN_FILE).is_file(),
        "the run wrote its report"
    );
    assert!(
        !report.dir.join(RUNNING_FILE).exists(),
        "a finished run is no longer marked as going"
    );
    let written: serde_json::Value =
        serde_json::from_slice(&fs::read(report.dir.join(RUN_FILE)).expect("readable"))
            .expect("the report decodes");
    assert_eq!(written["run"], report.run.to_string());
    assert_eq!(written["tasks"].as_array().map(Vec::len), Some(6));
    let product = fs::read(report.dir.join("gate").join("ledger#1").join(PRODUCT_FILE))
        .expect("the product is readable");
    assert_eq!(
        serde_json::from_slice::<Summary>(&product)
            .map(|summary| summary.line)
            .ok(),
        Some("41 42 clippy+test".to_string())
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_run_keeps_only_the_newest_runs_and_the_products_they_refer_to() {
    let root = root("keep");
    let plan = Plan::new("gate", &known(), &["costly"]).expect("valid");
    let runner = runner().keep(2);
    let first = runner
        .run(&plan, seeded("alpha", true), &root)
        .expect("seeded");
    runner
        .run(&plan, seeded("beta", true), &root)
        .expect("seeded");
    let third = runner
        .run(&plan, seeded("beta", true), &root)
        .expect("seeded");
    assert!(third.step("costly").expect("recorded").replayed);
    let runs: Vec<String> = fs::read_dir(root.join(RUNS_DIR))
        .expect("runs are listed")
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    assert_eq!(runs.len(), 2, "two runs are kept: {runs:?}");
    assert!(
        !runs.contains(&first.run.to_string()),
        "the oldest run is gone"
    );
    let stored = fs::read_dir(root.join(STORE_DIR))
        .expect("the store is listed")
        .count();
    assert_eq!(stored, 1, "only the product the kept runs refer to remains");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_chatty_worker_s_output_is_capped_on_disk_and_counted_whole() {
    let root = root("cap");
    let plan = Plan::new("gate", &known(), &["printer"]).expect("valid");
    let mut seeds = Store::new();
    seeds.put(&Chatter { bytes: 1 << 20 }).expect("encodes");
    let report = runner()
        .cap(1024)
        .run(&plan, seeds, &root)
        .expect("the chatter is seeded");
    assert_eq!(report.outcome, Outcome::Ok);
    let account = report
        .step("printer")
        .and_then(|step| step.account.as_ref())
        .expect("accounted");
    assert_eq!((account.stdout_bytes, account.stdout_kept), (1 << 20, 1024));
    let kept = fs::metadata(report.dir.join("gate").join("printer#1").join(STDOUT_FILE))
        .expect("the output file is there")
        .len();
    assert_eq!(kept, 1024);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_run_refuses_a_seed_of_a_type_a_task_produces() {
    let root = root("shadowed");
    let plan = Plan::new("gate", &known(), &["scan", "resolve"]).expect("the plan is valid");
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
        refused.problems,
        vec![Problem::Shadowed {
            need: Need::of::<Repo>(),
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
fn a_run_refuses_a_worker_that_is_not_an_executable_file() {
    let root = root("noworker");
    let plan = Plan::new("gate", &known(), &["scan"]).expect("the plan is valid");
    let missing = PathBuf::from("/nonexistent/worker");
    let refused = Runner::new()
        .worker(missing.clone())
        .run(&plan, seeded("repo", true), &root)
        .err()
        .expect("there is no worker to run");
    assert!(
        matches!(&refused.problems[..], [Problem::NoWorker { path, .. }] if path == &missing),
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
fn a_worker_that_cannot_start_fails_its_attempt() {
    let root = root("notaprogram");
    let plan = Plan::new("gate", &known(), &["scan"]).expect("valid");
    // An executable file whose interpreter does not exist: it passes
    // verification and the kernel refuses to start it.
    let worker = root.join("worker");
    fs::write(&worker, "#!/nonexistent/interpreter\n").expect("a file is written");
    fs::set_permissions(&worker, fs::Permissions::from_mode(0o755)).expect("made executable");
    let report = Runner::new()
        .worker(worker)
        .run(&plan, seeded("repo", true), &root)
        .expect("the run is verified");
    let scan = report.step("scan").expect("recorded");
    match &scan.outcome {
        Outcome::Failed(reason) => {
            assert!(reason.starts_with("the worker could not start"), "{reason}")
        }
        other => panic!("a missing worker did not fail the attempt: {other:?}"),
    }
    assert_eq!(scan.job.attempt, 1);
    assert!(scan.account.is_none(), "no process ran");
    let _ = fs::remove_dir_all(&root);
}
