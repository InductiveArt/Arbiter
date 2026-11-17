use std::any::type_name;
use std::path::Path;

use serde::de::DeserializeOwned;
use task_runner::contract::RunReport;
use task_runner::contract::TaskAttempt;
use task_runner::contract::TaskInvocation;
use task_runner::value::run_store::RunStore;

use crate::core::graph_data::GraphData;
use crate::core::task::Node;
use crate::core::task::Outcome;
use crate::domain::cargo_json::rendered;
use crate::domain::check_outcome::CheckOutcome;
use crate::domain::dials::Dials;
use crate::domain::ecosystem::Reader;
use crate::domain::finding::Finding;
use crate::domain::generated_checks::GeneratedChecks;
use crate::domain::law::LawFindings;
use crate::domain::planned_checks::PlannedChecks;
use crate::domain::subprocess::SubprocessExit;
use crate::domain::unit_check::UnitCheck;
use crate::ledger::LedgerAccount;
use crate::packs;
use crate::report::gate_run::GateRun;

// The task whose record tells how long each program took and whether the
// cache answered for it.
const PROGRAMS: &str = "run_subprocess";

// The gate run a plan made: the values its tasks left, and its record for what
// only the run knows, the time each program took and whether the cache
// answered for it. A value missing means its task did not pass, and no report
// is written from half a run.
pub fn gate_run(store: &RunStore, record: &RunReport) -> Result<GateRun, String> {
    let graph = left::<GraphData>(store)?.into_graph(packs::all())?;
    let planned = left::<PlannedChecks>(store)?;
    let exits = left::<Vec<SubprocessExit>>(store)?;
    let generated = left::<GeneratedChecks>(store)?;
    let dials = left::<Dials>(store)?;
    let laws = left::<Vec<LawFindings>>(store)?;
    let checked = left::<Vec<Finding>>(store)?;
    let ledger = left::<LedgerAccount>(store)?;
    // Unit by unit as the units stand, a unit's tools before its generated
    // code, each in the order planned.
    let place = |unit: &Path| graph.repo.units.iter().position(|held| held.path == unit);
    let mut tagged: Vec<(bool, CheckOutcome)> = ran(&planned.checks, &exits, record)?
        .into_iter()
        .map(|outcome| (false, outcome))
        .chain(
            generated
                .outcomes
                .into_iter()
                .map(|outcome| (true, outcome)),
        )
        .collect();
    tagged.sort_by_key(|(generated, outcome)| (place(&outcome.unit), *generated));
    let outcomes: Vec<CheckOutcome> = tagged.into_iter().map(|(_, outcome)| outcome).collect();
    let mut findings = dials.findings;
    findings.extend(laws.into_iter().flat_map(|law| law.findings));
    findings.extend(checked);
    findings.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    let mut notes = dials.notes;
    notes.extend(planned.notes);
    notes.extend(generated.notes);
    Ok(GateRun {
        graph,
        outcomes,
        notes,
        findings,
        ledger,
        steps: steps(record),
    })
}

// The run's steps as the court draws them, each task of the plan in order.
pub fn steps(record: &RunReport) -> Vec<Node> {
    record.invocations.iter().map(step).collect()
}

fn left<T: DeserializeOwned + 'static>(store: &RunStore) -> Result<T, String> {
    store
        .get::<T>()
        .ok_or_else(|| format!("the run left no {}", type_name::<T>()))
}

// How each check ended, from its own program's exit, in the order the checks
// were planned. The exits come as the checks list their programs.
fn ran(
    checks: &[UnitCheck],
    exits: &[SubprocessExit],
    record: &RunReport,
) -> Result<Vec<CheckOutcome>, String> {
    let listed: usize = checks.iter().map(|check| check.subprocesses().len()).sum();
    if exits.len() != listed {
        return Err(format!(
            "the checks list {listed} programs, and the run left the exits of {}",
            exits.len()
        ));
    }
    let programs = record.invocation(PROGRAMS);
    let mut exits = exits.iter();
    let mut outcomes = Vec::with_capacity(checks.len());
    for check in checks {
        let mine: Vec<&SubprocessExit> = exits.by_ref().take(check.subprocesses().len()).collect();
        let exit = mine[0];
        let attempt = programs.and_then(|programs| item(programs, &check.subprocess.name));
        let passed = exit.exited_with_zero();
        outcomes.push(CheckOutcome {
            check: check.check.clone(),
            unit: check.unit.clone(),
            language: check.ecosystem.clone(),
            passed,
            seconds: attempt.map_or(0.0, program_seconds),
            output: if passed {
                String::new()
            } else {
                readable(check, exit)
            },
            warm: attempt.is_some_and(|attempt| attempt.replayed),
        });
    }
    Ok(outcomes)
}

// All a check's program printed, as a person reads it: what its reader
// renders of what it printed, then what it printed beside.
fn readable(check: &UnitCheck, exit: &SubprocessExit) -> String {
    let printed = match check.reading.as_ref().map(|reading| reading.reader) {
        None => exit.stdout.clone(),
        Some(Reader::CargoJson) => rendered(&exit.stdout),
    };
    format!("{printed}{}", exit.stderr)
}

// The last attempt of the item a fanned task ran under the key.
fn item<'record>(fanned: &'record TaskInvocation, key: &str) -> Option<&'record TaskAttempt> {
    let path = format!("{}@{key}", fanned.id().path);
    fanned
        .instances
        .iter()
        .find(|instance| instance.id().path == path)
        .and_then(|instance| instance.attempts.last())
}

// The time the program itself took; a replayed attempt ran none.
fn program_seconds(attempt: &TaskAttempt) -> f64 {
    attempt
        .resources()
        .processes
        .first()
        .map_or(0.0, |process| process.wall_nanos as f64 / 1e9)
}

// One task of the run as the court draws it: its name, or its item's key for
// a task run per item, how its last attempt ended, its cost, what it noted,
// and each item under it. A task run per item costs what its items cost.
fn step(invocation: &TaskInvocation) -> Node {
    let last = invocation.attempts.last();
    let children: Vec<Node> = invocation.instances.iter().map(step).collect();
    let seconds = if children.is_empty() {
        last.map_or(0.0, TaskAttempt::seconds)
    } else {
        children.iter().map(|child| child.seconds).sum()
    };
    let path = &invocation.id().path;
    Node {
        op: path
            .split_once('@')
            .map_or_else(|| invocation.task.clone(), |(_, key)| key.to_string()),
        outcome: if last.is_some_and(|attempt| attempt.outcome().is_failure()) {
            Outcome::Failed
        } else {
            Outcome::Ok
        },
        seconds,
        evidence: last
            .map(|attempt| attempt.evidence.clone())
            .unwrap_or_default(),
        children,
    }
}
