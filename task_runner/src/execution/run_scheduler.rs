use std::any::TypeId;
use std::collections::HashMap;
use std::collections::HashSet;
use std::time::SystemTime;

use crate::contract::run_record::Ended;
use crate::contract::RunId;
use crate::contract::RunRecord;
use crate::contract::TaskAttempt;
use crate::contract::TaskAttemptId;
use crate::contract::TaskInvocation;
use crate::contract::TaskInvocationId;
use crate::declaration::fan_out::FanOut;
use crate::declaration::plan::Plan;
use crate::declaration::read::Read;
use crate::declaration::read::ReadInput;
use crate::declaration::read::When;
use crate::declaration::task::Task;
use crate::execution::decision::Decision;
use crate::kernel::clock::unix_nanos;
use crate::record::completion::Completion;
use crate::value::encoded_value::EncodedValue;
use crate::value::identity::Identity;
use crate::value::run_store::RunStore;
use crate::value::task_cache_key::TaskCacheKey;
use crate::value::task_output_hash::TaskOutputHash;

// The run's bookkeeping, with nothing in it that waits, reads a file or
// starts a process: what waits on what, every invocation made so far with its
// attempts, which are asked for and not yet heard of, the items of each
// fanned task, how the producer of each type finished, and the values held.
// Fed what happened, it answers with what should happen next. The first
// invocations are the plan's tasks, in plan order.
pub struct RunScheduler<'a> {
    plan: &'a Plan,
    store: RunStore,
    completions: HashMap<TypeId, Completion>,
    identity: Identity,
    run: RunId,
    waits: Vec<usize>,
    left: usize,
    invocations: Vec<Invocation>,
    fanned: Vec<Option<Fanned>>,
    cancelled: bool,
}

// One invocation: of a plan task by its position, or of one item of a fanned
// task; the attempts made so far, how many tries were spent, and whether one
// is asked for and not yet heard of.
struct Invocation {
    at: usize,
    item: Option<Item>,
    tried: u32,
    asked: bool,
    attempts: Vec<TaskAttempt>,
}

// One item of a fanned task: its place in the list, which orders the gathered
// outputs, the key it goes by in the record, and its value.
struct Item {
    place: usize,
    key: String,
    value: EncodedValue,
}

// A fanned task once its list arrived: the invocation of each item, how many
// have yet to settle, and what each left.
struct Fanned {
    instances: Vec<usize>,
    unsettled: usize,
    outputs: Vec<Option<EncodedValue>>,
}

impl<'a> RunScheduler<'a> {
    // Begins the run from the seeds: every task whose reads are all held is
    // decided at once.
    pub fn new(
        plan: &'a Plan,
        seeds: RunStore,
        identity: Identity,
        run: RunId,
    ) -> (RunScheduler<'a>, Vec<Decision>) {
        let count = plan.tasks.len();
        let mut scheduler = RunScheduler {
            plan,
            store: seeds,
            completions: HashMap::new(),
            identity,
            run,
            waits: plan.waits(),
            left: count,
            invocations: (0..count)
                .map(|at| Invocation {
                    at,
                    item: None,
                    tried: 0,
                    asked: false,
                    attempts: Vec::new(),
                })
                .collect(),
            fanned: (0..count).map(|_| None).collect(),
            cancelled: false,
        };
        let mut decisions = Vec::new();
        for at in 0..count {
            if scheduler.waits[at] == 0 {
                scheduler.ready(at, &mut decisions);
            }
        }
        (scheduler, decisions)
    }

    pub fn plan(&self) -> &'a Plan {
        self.plan
    }

    pub fn task_of(&self, invocation: usize) -> usize {
        self.invocations[invocation].at
    }

    // The key of the item an invocation is of; none for the invocation of a
    // plan task.
    pub fn item_of(&self, invocation: usize) -> Option<&str> {
        self.invocations[invocation]
            .item
            .as_ref()
            .map(|item| item.key.as_str())
    }

    pub fn invocation_id(&self, invocation: usize) -> TaskInvocationId {
        let made = &self.invocations[invocation];
        let task = &self.plan.tasks[made.at].name;
        let path = match &made.item {
            None => format!("{}/{task}", self.plan.name),
            Some(item) => format!("{}/{task}@{}", self.plan.name, item.key),
        };
        TaskInvocationId {
            run: Some(self.run),
            path,
        }
    }

    pub fn attempt_id(&self, invocation: usize, attempt: u32) -> TaskAttemptId {
        TaskAttemptId {
            invocation: Some(self.invocation_id(invocation)),
            attempt,
        }
    }

    // The inputs an attempt is handed, in its task's reads' order: an item of
    // a fanned task is handed its own item where the task reads one.
    pub fn read_inputs(&self, invocation: usize) -> Vec<ReadInput> {
        let made = &self.invocations[invocation];
        let task = &self.plan.tasks[made.at];
        let fan_out = self.plan.fan_out(made.at);
        task.reads
            .iter()
            .map(|read| match (fan_out, &made.item) {
                (Some(fan_out), Some(item)) if read.value_type == fan_out.item => ReadInput {
                    value: Some(item.value.clone()),
                    completion: None,
                },
                _ => input_for(read, &self.store, &self.completions),
            })
            .collect()
    }

    pub fn settled(&self) -> bool {
        self.left == 0
    }

    // Every plan task's invocation in plan order, a fanned task's items inside
    // it, and the values held.
    pub fn records(self) -> (Vec<TaskInvocation>, RunStore) {
        let ids: Vec<TaskInvocationId> = (0..self.invocations.len())
            .map(|invocation| self.invocation_id(invocation))
            .collect();
        let plan = self.plan;
        let mut made: Vec<Option<TaskInvocation>> = ids
            .into_iter()
            .zip(self.invocations)
            .map(|(id, invocation)| {
                Some(TaskInvocation {
                    id: Some(id),
                    task: plan.tasks[invocation.at].name.clone(),
                    attempts: invocation.attempts,
                    instances: Vec::new(),
                })
            })
            .collect();
        let mut recorded = Vec::with_capacity(plan.tasks.len());
        for at in 0..plan.tasks.len() {
            let Some(mut task) = made[at].take() else {
                continue;
            };
            if let Some(fanned) = &self.fanned[at] {
                task.instances = fanned
                    .instances
                    .iter()
                    .filter_map(|&instance| made[instance].take())
                    .collect();
            }
            recorded.push(task);
        }
        (recorded, self.store)
    }

    // The cache named an output for the key and the store still holds it:
    // the invocation is recorded as replayed, attempt 0, naming both, and
    // passes with it.
    pub fn replayed(
        &mut self,
        invocation: usize,
        key: TaskCacheKey,
        hash: TaskOutputHash,
        output: EncodedValue,
    ) -> Vec<Decision> {
        let mut out = Vec::new();
        let mut attempt = TaskAttempt::unrun(self.attempt_id(invocation, 0), RunRecord::passed());
        attempt.began_unix_nanos = Some(unix_nanos(SystemTime::now()));
        attempt.replayed = true;
        attempt.output = Some(hash.as_str().to_string());
        attempt.cache_key = Some(key.as_str().to_string());
        self.invocations[invocation].asked = false;
        self.record(invocation, attempt, &mut out);
        self.settle(invocation, RunRecord::passed(), Some(output), &mut out);
        out
    }

    // An attempt is over: the invocation passed, earns another attempt, or is done.
    pub fn ended(
        &mut self,
        invocation: usize,
        attempt: TaskAttempt,
        output: Option<EncodedValue>,
    ) -> Vec<Decision> {
        let mut out = Vec::new();
        let outcome = attempt.outcome().clone();
        let retries = self.plan.tasks[self.invocations[invocation].at].retries;
        let tried = attempt.id().attempt;
        let made = &mut self.invocations[invocation];
        if tried > 0 {
            made.tried = made.tried.max(tried);
        }
        made.asked = false;
        let again = outcome.is_failure()
            && outcome != RunRecord::cancelled()
            && !self.cancelled
            && made.tried <= retries;
        self.record(invocation, attempt, &mut out);
        if outcome == RunRecord::passed() {
            self.settle(invocation, outcome, output, &mut out);
        } else if again {
            self.ask(invocation, &mut out);
        } else {
            self.settle(invocation, outcome, None, &mut out);
        }
        out
    }

    // The run was cancelled: nothing earns an attempt after this, and every
    // attempt asked for and not yet heard of is to be stopped.
    pub fn cancel(&mut self) -> Vec<Decision> {
        self.cancelled = true;
        (0..self.invocations.len())
            .filter(|&invocation| self.invocations[invocation].asked)
            .map(|invocation| Decision::Stop { invocation })
            .collect()
    }

    // An attempt asked for was never started: the invocation ends as cancelled.
    pub fn withdrawn(&mut self, invocation: usize) -> Vec<Decision> {
        let mut out = Vec::new();
        self.invocations[invocation].asked = false;
        let attempt = TaskAttempt::unrun(self.attempt_id(invocation, 0), RunRecord::cancelled());
        self.record(invocation, attempt, &mut out);
        self.settle(invocation, RunRecord::cancelled(), None, &mut out);
        out
    }

    // Every producer of the task has settled: its outcome is already known and
    // it is recorded without an attempt, a fanned task is handed its list, or
    // an attempt is wanted.
    fn ready(&mut self, at: usize, out: &mut Vec<Decision>) {
        let plan = self.plan;
        if let Some(outcome) = outcome_from_producers(&plan.reads(at), &self.completions) {
            let attempt = TaskAttempt::unrun(self.attempt_id(at, 0), outcome.clone());
            self.record(at, attempt, out);
            self.settle(at, outcome, None, out);
            return;
        }
        if plan.fan_out(at).is_some() {
            self.fan_out(at, out);
            return;
        }
        self.ask(at, out);
    }

    // A fanned task's list arrived: one invocation for each item, each asked
    // for, the task settling once every item has. A list that is not one, or
    // whose items do not each go by a key of their own, fails the task.
    fn fan_out(&mut self, at: usize, out: &mut Vec<Decision>) {
        let plan = self.plan;
        let Some(fan_out) = plan.fan_out(at) else {
            return;
        };
        let items = match self.store.value(fan_out.list.id) {
            None => Err("no list was handed".to_string()),
            Some(list) => items_of(list).and_then(|items| keyed(fan_out, items)),
        };
        let items = match items {
            Ok(items) => items,
            Err(reason) => {
                let outcome = RunRecord::failed(format!(
                    "{} could not be fanned out: {reason}",
                    fan_out.list.name
                ));
                let attempt = TaskAttempt::unrun(self.attempt_id(at, 0), outcome.clone());
                self.record(at, attempt, out);
                self.settle(at, outcome, None, out);
                return;
            }
        };
        let mut instances = Vec::with_capacity(items.len());
        for (place, (key, value)) in items.into_iter().enumerate() {
            self.invocations.push(Invocation {
                at,
                item: Some(Item { place, key, value }),
                tried: 0,
                asked: false,
                attempts: Vec::new(),
            });
            instances.push(self.invocations.len() - 1);
        }
        let count = instances.len();
        self.fanned[at] = Some(Fanned {
            instances: instances.clone(),
            unsettled: count,
            outputs: vec![None; count],
        });
        if count == 0 {
            self.gather(at, out);
        }
        for instance in instances {
            self.ask(instance, out);
        }
    }

    fn ask(&mut self, invocation: usize, out: &mut Vec<Decision>) {
        let plan = self.plan;
        let task = &plan.tasks[self.invocations[invocation].at];
        let key = task
            .caching
            .replays()
            .then(|| self.key(task, &self.read_inputs(invocation)));
        self.invocations[invocation].asked = true;
        out.push(Decision::Attempt {
            invocation,
            attempt: self.invocations[invocation].tried + 1,
            key,
        });
    }

    fn record(&mut self, invocation: usize, attempt: TaskAttempt, out: &mut Vec<Decision>) {
        out.push(Decision::Recorded {
            invocation,
            attempt: Box::new(attempt.clone()),
        });
        self.invocations[invocation].attempts.push(attempt);
    }

    // An invocation's outcome is final: an item's counts toward its fanned
    // task, which settles once every item has; a plan task's settles the task.
    fn settle(
        &mut self,
        invocation: usize,
        outcome: RunRecord,
        output: Option<EncodedValue>,
        out: &mut Vec<Decision>,
    ) {
        out.push(Decision::Settled {
            invocation,
            outcome: outcome.clone(),
        });
        let at = self.invocations[invocation].at;
        let Some(place) = self.invocations[invocation]
            .item
            .as_ref()
            .map(|item| item.place)
        else {
            self.settle_task(at, outcome, output, out);
            return;
        };
        let Some(fanned) = self.fanned[at].as_mut() else {
            return;
        };
        fanned.outputs[place] = output;
        fanned.unsettled -= 1;
        if fanned.unsettled == 0 {
            self.gather(at, out);
        }
    }

    // Every item of a fanned task has settled: the task passes with their
    // outputs gathered in item order when each passed, and fails naming those
    // that did not otherwise. Its attempt 0 records which.
    fn gather(&mut self, at: usize, out: &mut Vec<Decision>) {
        let Some(fanned) = &self.fanned[at] else {
            return;
        };
        let missed: Vec<String> = fanned
            .instances
            .iter()
            .filter(|&&instance| {
                self.invocations[instance]
                    .attempts
                    .last()
                    .is_none_or(|attempt| *attempt.outcome() != RunRecord::passed())
            })
            .map(|&instance| self.invocation_id(instance).path)
            .collect();
        let (outcome, output) = if missed.is_empty() {
            (RunRecord::passed(), Some(gathered(&fanned.outputs)))
        } else {
            (
                RunRecord::failed(format!(
                    "{} of {} items did not pass: {}",
                    missed.len(),
                    fanned.instances.len(),
                    missed.join(", ")
                )),
                None,
            )
        };
        let mut attempt = TaskAttempt::unrun(self.attempt_id(at, 0), outcome.clone());
        attempt.began_unix_nanos = Some(unix_nanos(SystemTime::now()));
        self.record(at, attempt, out);
        self.settle(at, outcome, output, out);
    }

    // A plan task's outcome is final: what it left is held for its readers,
    // and every reader that waited only on it is now ready.
    fn settle_task(
        &mut self,
        at: usize,
        outcome: RunRecord,
        output: Option<EncodedValue>,
        out: &mut Vec<Decision>,
    ) {
        self.left -= 1;
        // A task leaving nothing has no type to be held under: its record is
        // all it leaves.
        if !self.plan.tasks[at].leaves_nothing() {
            let attempt_id = self.invocations[at]
                .attempts
                .last()
                .expect("a settled task has a record")
                .id()
                .clone();
            let id = self.plan.produces(at).id;
            self.completions.insert(
                id,
                Completion {
                    attempt_id,
                    outcome,
                },
            );
            if let Some(output) = output {
                self.store.insert(id, output);
            }
        }
        let plan = self.plan;
        for &next in plan.dependents(at) {
            self.waits[next] -= 1;
            if self.waits[next] == 0 {
                self.ready(next, out);
            }
        }
    }

    // What an output is cached under: the task executable, the task's name and
    // every input, so the same code on the same inputs finds it again.
    fn key(&self, task: &Task, inputs: &[ReadInput]) -> TaskCacheKey {
        let whens: Vec<String> = task
            .reads
            .iter()
            .map(|read| format!("{:?}", read.when))
            .collect();
        let outcomes: Vec<Option<String>> = inputs
            .iter()
            .map(|input| {
                input
                    .completion
                    .as_ref()
                    .map(|completion| format!("{:?}", completion.outcome))
            })
            .collect();
        let mut parts: Vec<Option<&[u8]>> = vec![
            Some(self.identity.as_str().as_bytes()),
            Some(task.name.as_bytes()),
        ];
        for ((when, input), outcome) in whens.iter().zip(inputs).zip(&outcomes) {
            parts.push(Some(when.as_bytes()));
            parts.push(input.value.as_ref().map(|value| &*value.bytes));
            parts.push(outcome.as_deref().map(str::as_bytes));
        }
        TaskCacheKey::of(&parts)
    }
}

// What a reader's outcome already is, from how its producers finished, or
// nothing when every read fires and it has to run. A skipped producer skips
// the reader before a branch not taken is counted, and both carry the path of
// the task that decided them. A seeded type, having no producer, always
// fires. Asked once every producer has finished.
fn outcome_from_producers(
    reads: &[Read],
    completions: &HashMap<TypeId, Completion>,
) -> Option<RunRecord> {
    let mut unneeded: Option<String> = None;
    for read in reads {
        let Some(completion) = completions.get(&read.value_type.id) else {
            continue;
        };
        if read.when == When::Finished {
            continue;
        }
        match &completion.outcome.ended {
            Some(Ended::Skipped(because)) => return Some(RunRecord::skipped(because.clone())),
            Some(Ended::Unneeded(because)) => {
                unneeded.get_or_insert_with(|| because.clone());
            }
            _ if read.when == When::Passed && completion.outcome.is_failure() => {
                return Some(RunRecord::skipped(completion.attempt_id.path()));
            }
            Some(Ended::Passed(_)) if read.when == When::Failed => {
                unneeded.get_or_insert_with(|| completion.attempt_id.path().to_string());
            }
            _ => {}
        }
    }
    unneeded.map(RunRecord::unneeded)
}

// The input a read is handed. Asked only once the reader is to run, so every
// input the read needs is present.
fn input_for(
    read: &Read,
    store: &RunStore,
    completions: &HashMap<TypeId, Completion>,
) -> ReadInput {
    let value = store.value(read.value_type.id).cloned();
    let completion = completions.get(&read.value_type.id).cloned();
    match read.when {
        When::Passed => ReadInput {
            value,
            completion: None,
        },
        When::Failed => ReadInput {
            value: None,
            completion,
        },
        When::Finished => ReadInput { value, completion },
    }
}

// The items of a list, each as a value of its own.
fn items_of(list: &EncodedValue) -> Result<Vec<EncodedValue>, String> {
    let items: Vec<serde_json::Value> =
        serde_json::from_slice(&list.bytes).map_err(|error| error.to_string())?;
    items
        .iter()
        .map(|item| EncodedValue::encoded(item).map_err(|error| error.to_string()))
        .collect()
}

// Each item with the key it goes by. Two items sharing a key would share a
// name in the record, so such a list is refused, as is an empty key.
fn keyed(
    fan_out: &FanOut,
    items: Vec<EncodedValue>,
) -> Result<Vec<(String, EncodedValue)>, String> {
    let mut seen = HashSet::new();
    let mut keyed = Vec::with_capacity(items.len());
    for (place, value) in items.into_iter().enumerate() {
        let key = fan_out.key_of(&value).map_err(|error| {
            format!(
                "item {place} does not read as {}: {error}",
                fan_out.item.name
            )
        })?;
        if key.is_empty() {
            return Err(format!("item {place} goes by an empty key"));
        }
        if !seen.insert(key.clone()) {
            return Err(format!("two items share the key {key}"));
        }
        keyed.push((key, value));
    }
    Ok(keyed)
}

// The outputs of a fanned task's items as one list, in item order.
fn gathered(outputs: &[Option<EncodedValue>]) -> EncodedValue {
    let mut bytes = vec![b'['];
    for (n, output) in outputs.iter().flatten().enumerate() {
        if n > 0 {
            bytes.push(b',');
        }
        bytes.extend_from_slice(&output.bytes);
    }
    bytes.push(b']');
    EncodedValue {
        bytes: bytes.into(),
        file: None,
    }
}

#[cfg(test)]
mod tests {
    use std::any::TypeId;
    use std::collections::HashMap;

    use serde::Deserialize;
    use serde::Serialize;

    use super::input_for;
    use super::outcome_from_producers;
    use super::RunScheduler;
    use crate::context::task_context::TaskContext;
    use crate::contract::run_record::Ended;
    use crate::contract::RunId;
    use crate::contract::RunRecord;
    use crate::contract::TaskAttempt;
    use crate::contract::TaskAttemptId;
    use crate::declaration::fan_out::FanOut;
    use crate::declaration::plan::Plan;
    use crate::declaration::pull::Failed;
    use crate::declaration::pull::In;
    use crate::declaration::read::Read;
    use crate::declaration::read::ReadInput;
    use crate::declaration::read::When;
    use crate::declaration::task::Task;
    use crate::execution::decision::Decision;
    use crate::record::completion::Completion;
    use crate::record::task_failure::TaskFailure;
    use crate::value::encoded_value::EncodedValue;
    use crate::value::identity::Identity;
    use crate::value::run_store::RunStore;
    use crate::value::task_output_hash::TaskOutputHash;

    #[derive(Serialize, Deserialize)]
    struct Root(String);
    #[derive(Serialize, Deserialize)]
    struct Repo;
    #[derive(Serialize, Deserialize)]
    struct Graph;
    #[derive(Serialize, Deserialize)]
    struct Fixed;
    #[derive(Serialize, Deserialize)]
    struct Number(i64);
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Squared(i64);
    #[derive(Serialize, Deserialize)]
    struct Total(i64);

    struct A;
    struct B;
    struct C;
    struct Seed;

    fn scan() -> Task {
        Task::new("scan", |_: &mut TaskContext, _: In<Root>| {
            Ok::<Repo, TaskFailure>(Repo)
        })
    }

    fn resolve() -> Task {
        Task::new("resolve", |_: &mut TaskContext, _: In<Repo>| {
            Ok::<Graph, TaskFailure>(Graph)
        })
    }

    fn fix() -> Task {
        Task::new("fix", |_: &mut TaskContext, _: Failed<Repo>| {
            Ok::<Fixed, TaskFailure>(Fixed)
        })
    }

    fn list() -> Task {
        Task::new("list", |_: &mut TaskContext, _: In<Root>| {
            Ok::<Vec<Number>, TaskFailure>(Vec::new())
        })
    }

    fn square() -> Task {
        Task::new("square", |_: &mut TaskContext, number: In<Number>| {
            Ok::<Squared, TaskFailure>(Squared(number.0 * number.0))
        })
    }

    fn sum() -> Task {
        Task::new("sum", |_: &mut TaskContext, squares: In<Vec<Squared>>| {
            Ok::<Total, TaskFailure>(Total(squares.iter().map(|square| square.0).sum()))
        })
    }

    // The list, its squares one item at a time, and their sum.
    fn squares(known: &[Task]) -> Plan {
        Plan::fanned(
            "gate",
            known,
            &["list", "square", "sum"],
            &[FanOut::of::<Number, Squared>("square", |number| {
                number.0.to_string()
            })],
        )
        .expect("valid")
    }

    // The list task passed, leaving these numbers.
    fn listed(scheduler: &mut RunScheduler<'_>, numbers: &[i64]) -> Vec<Decision> {
        let numbers: Vec<Number> = numbers.iter().map(|&n| Number(n)).collect();
        let attempt = passed(scheduler, 0, 1);
        scheduler.ended(
            0,
            attempt,
            Some(EncodedValue::encoded(&numbers).expect("encodes")),
        )
    }

    fn gathered(scheduler: &RunScheduler<'_>, reader: usize) -> Option<Vec<Squared>> {
        scheduler.read_inputs(reader)[0]
            .value
            .as_ref()
            .and_then(|value| value.decoded().ok())
    }

    fn seeds() -> RunStore {
        let mut store = RunStore::new();
        store.put(&Root("repo".to_string())).expect("encodes");
        store
    }

    fn begun(plan: &Plan) -> (RunScheduler<'_>, Vec<Decision>) {
        RunScheduler::new(plan, seeds(), Identity::of(b"7"), RunId::next())
    }

    // The record a process attempt leaves, as the executor hands it back.
    fn recorded(
        scheduler: &RunScheduler<'_>,
        invocation: usize,
        attempt: u32,
        outcome: RunRecord,
    ) -> TaskAttempt {
        TaskAttempt::unrun(scheduler.attempt_id(invocation, attempt), outcome)
    }

    fn passed(scheduler: &RunScheduler<'_>, invocation: usize, attempt: u32) -> TaskAttempt {
        recorded(scheduler, invocation, attempt, RunRecord::passed())
    }

    fn failed(scheduler: &RunScheduler<'_>, invocation: usize, attempt: u32) -> TaskAttempt {
        recorded(
            scheduler,
            invocation,
            attempt,
            RunRecord::failed("no".to_string()),
        )
    }

    fn repo() -> EncodedValue {
        EncodedValue::encoded(&Repo).expect("encodes")
    }

    fn nothing(name: &str) -> Task {
        Task::new(
            name,
            |_: &mut TaskContext, (): ()| Ok::<(), TaskFailure>(()),
        )
    }

    fn finished<T: 'static>(
        completions: &mut HashMap<TypeId, Completion>,
        path: &str,
        outcome: RunRecord,
    ) {
        completions.insert(
            TypeId::of::<T>(),
            Completion {
                attempt_id: TaskAttemptId::new(RunId::next(), path, 1),
                outcome,
            },
        );
    }

    #[test]
    fn a_scheduler_keeps_no_value_for_a_task_leaving_nothing() {
        let known = [nothing("one"), nothing("two")];
        let plan = Plan::new("gate", &known, &["one", "two"]).expect("valid");
        let (mut scheduler, _) = begun(&plan);
        let attempt = passed(&scheduler, 0, 1);
        scheduler.ended(
            0,
            attempt,
            Some(EncodedValue::encoded(&()).expect("encodes")),
        );
        let attempt = failed(&scheduler, 1, 1);
        scheduler.ended(1, attempt, None);
        assert!(scheduler.settled());
        let (_, store) = scheduler.records();
        assert!(!store.has(TypeId::of::<()>()));
    }

    #[test]
    fn a_scheduler_asks_first_for_the_tasks_whose_reads_are_all_held() {
        let plan = Plan::new("gate", &[scan(), resolve()], &["scan", "resolve"]).expect("valid");
        let (scheduler, decisions) = begun(&plan);
        assert_eq!(
            decisions,
            [Decision::Attempt {
                invocation: 0,
                attempt: 1,
                key: None
            }]
        );
        assert!(!scheduler.settled());
    }

    #[test]
    fn a_scheduler_asks_for_a_reader_the_moment_its_producer_passes() {
        let plan = Plan::new("gate", &[scan(), resolve()], &["scan", "resolve"]).expect("valid");
        let (mut scheduler, _) = begun(&plan);
        let attempt = passed(&scheduler, 0, 1);
        let decisions = scheduler.ended(0, attempt.clone(), Some(repo()));
        assert_eq!(
            decisions,
            [
                Decision::Recorded {
                    invocation: 0,
                    attempt: Box::new(attempt)
                },
                Decision::Settled {
                    invocation: 0,
                    outcome: RunRecord::passed()
                },
                Decision::Attempt {
                    invocation: 1,
                    attempt: 1,
                    key: None
                },
            ]
        );
        let decisions = scheduler.ended(
            1,
            passed(&scheduler, 1, 1),
            Some(EncodedValue::encoded(&Graph).expect("encodes")),
        );
        assert!(matches!(
            decisions.last(),
            Some(Decision::Settled { invocation: 1, .. })
        ));
        assert!(scheduler.settled());
        let (invocations, store) = scheduler.records();
        assert_eq!(
            invocations
                .iter()
                .map(|invocation| invocation.attempts.len())
                .collect::<Vec<_>>(),
            [1, 1]
        );
        assert!(store.get::<Graph>().is_some());
    }

    #[test]
    fn a_scheduler_skips_the_readers_of_a_failed_task_and_asks_for_its_failure_readers() {
        let plan = Plan::new(
            "gate",
            &[scan(), resolve(), fix()],
            &["scan", "resolve", "fix"],
        )
        .expect("valid");
        let (mut scheduler, _) = begun(&plan);
        let decisions = scheduler.ended(0, failed(&scheduler, 0, 1), None);
        let skipped = TaskAttempt::unrun(
            scheduler.attempt_id(1, 0),
            RunRecord::skipped("gate/scan".to_string()),
        );
        assert_eq!(
            decisions[1..],
            [
                Decision::Settled {
                    invocation: 0,
                    outcome: RunRecord::failed("no".to_string())
                },
                Decision::Recorded {
                    invocation: 1,
                    attempt: Box::new(skipped)
                },
                Decision::Settled {
                    invocation: 1,
                    outcome: RunRecord::skipped("gate/scan".to_string())
                },
                Decision::Attempt {
                    invocation: 2,
                    attempt: 1,
                    key: None
                },
            ]
        );
    }

    #[test]
    fn a_scheduler_asks_again_after_a_failure_until_the_retries_are_spent() {
        let plan = Plan::new("gate", &[scan().retries(1)], &["scan"]).expect("valid");
        let (mut scheduler, _) = begun(&plan);
        let decisions = scheduler.ended(0, failed(&scheduler, 0, 1), None);
        assert_eq!(
            decisions[1],
            Decision::Attempt {
                invocation: 0,
                attempt: 2,
                key: None
            }
        );
        let decisions = scheduler.ended(0, failed(&scheduler, 0, 2), None);
        assert_eq!(
            decisions[1],
            Decision::Settled {
                invocation: 0,
                outcome: RunRecord::failed("no".to_string())
            }
        );
        assert!(scheduler.settled());
    }

    #[test]
    fn a_scheduler_cancelled_stops_what_was_asked_and_a_withdrawn_attempt_ends_as_attempt_zero() {
        let plan = Plan::new("gate", &[scan(), fix().retries(2)], &["scan", "fix"]).expect("valid");
        let (mut scheduler, _) = begun(&plan);
        assert_eq!(scheduler.cancel(), [Decision::Stop { invocation: 0 }]);
        let decisions = scheduler.withdrawn(0);
        let cancelled = TaskAttempt::unrun(scheduler.attempt_id(0, 0), RunRecord::cancelled());
        assert_eq!(
            decisions,
            [
                Decision::Recorded {
                    invocation: 0,
                    attempt: Box::new(cancelled)
                },
                Decision::Settled {
                    invocation: 0,
                    outcome: RunRecord::cancelled()
                },
                Decision::Attempt {
                    invocation: 1,
                    attempt: 1,
                    key: None
                },
            ]
        );
        // What the cancel unlocked still runs, but a failure earns no retry now.
        let decisions = scheduler.ended(1, failed(&scheduler, 1, 1), None);
        assert_eq!(
            decisions[1],
            Decision::Settled {
                invocation: 1,
                outcome: RunRecord::failed("no".to_string())
            }
        );
        assert!(scheduler.settled());
    }

    #[test]
    fn a_fanned_task_asks_for_every_item_at_once_and_gathers_their_outputs_in_item_order() {
        let known = [list(), square(), sum()];
        let plan = squares(&known);
        let (mut scheduler, _) = begun(&plan);
        let decisions = listed(&mut scheduler, &[1, 2, 3]);
        assert_eq!(
            decisions[2..],
            [3, 4, 5].map(|invocation| Decision::Attempt {
                invocation,
                attempt: 1,
                key: None
            })
        );
        assert_eq!(
            (3..6)
                .map(|item| scheduler.invocation_id(item).path)
                .collect::<Vec<_>>(),
            ["gate/square@1", "gate/square@2", "gate/square@3"],
            "an item goes by its key"
        );
        let handed: Option<Number> = scheduler.read_inputs(4)[0]
            .value
            .as_ref()
            .and_then(|value| value.decoded().ok());
        assert_eq!(
            handed.map(|number| number.0),
            Some(2),
            "an item is handed its own value"
        );
        // The items end out of order; the gathering keeps theirs.
        for (item, value) in [(5, 9), (3, 1)] {
            let attempt = passed(&scheduler, item, 1);
            let decisions = scheduler.ended(
                item,
                attempt,
                Some(EncodedValue::encoded(&Squared(value)).expect("encodes")),
            );
            assert_eq!(
                decisions.len(),
                2,
                "an item settling alone settles nothing else"
            );
        }
        let attempt = passed(&scheduler, 4, 1);
        let decisions = scheduler.ended(
            4,
            attempt,
            Some(EncodedValue::encoded(&Squared(4)).expect("encodes")),
        );
        match &decisions[2] {
            Decision::Recorded {
                invocation: 1,
                attempt,
            } => assert_eq!(
                (attempt.id().attempt, attempt.outcome()),
                (0, &RunRecord::passed())
            ),
            other => panic!("the gathering was not recorded as attempt 0: {other:?}"),
        }
        assert_eq!(
            decisions[3..],
            [
                Decision::Settled {
                    invocation: 1,
                    outcome: RunRecord::passed()
                },
                Decision::Attempt {
                    invocation: 2,
                    attempt: 1,
                    key: None
                },
            ]
        );
        assert_eq!(
            gathered(&scheduler, 2),
            Some(vec![Squared(1), Squared(4), Squared(9)])
        );
        let attempt = passed(&scheduler, 2, 1);
        scheduler.ended(
            2,
            attempt,
            Some(EncodedValue::encoded(&Total(14)).expect("encodes")),
        );
        assert!(scheduler.settled());
        let (invocations, _) = scheduler.records();
        assert_eq!(
            invocations
                .iter()
                .map(|invocation| (invocation.task.as_str(), invocation.instances.len()))
                .collect::<Vec<_>>(),
            [("list", 0), ("square", 3), ("sum", 0)],
            "the items are recorded inside their task"
        );
    }

    #[test]
    fn a_fanned_task_whose_items_share_a_key_fails_before_any_item_runs() {
        let known = [list(), square(), sum()];
        let plan = squares(&known);
        let (mut scheduler, _) = begun(&plan);
        let decisions = listed(&mut scheduler, &[2, 3, 2]);
        let reason = match &decisions[3] {
            Decision::Settled {
                invocation: 1,
                outcome:
                    RunRecord {
                        ended: Some(Ended::Failed(reason)),
                    },
            } => reason.clone(),
            other => panic!("the fanned task did not fail: {other:?}"),
        };
        assert!(
            reason.ends_with("could not be fanned out: two items share the key 2"),
            "{reason}"
        );
        assert!(
            !decisions
                .iter()
                .any(|decision| matches!(decision, Decision::Attempt { .. })),
            "no item is asked for"
        );
    }

    #[test]
    fn a_fanned_task_over_an_empty_list_passes_at_once_with_an_empty_list() {
        let known = [list(), square(), sum()];
        let plan = squares(&known);
        let (mut scheduler, _) = begun(&plan);
        let decisions = listed(&mut scheduler, &[]);
        assert_eq!(
            decisions[3..],
            [
                Decision::Settled {
                    invocation: 1,
                    outcome: RunRecord::passed()
                },
                Decision::Attempt {
                    invocation: 2,
                    attempt: 1,
                    key: None
                },
            ]
        );
        assert_eq!(gathered(&scheduler, 2), Some(Vec::new()));
    }

    #[test]
    fn a_scheduler_keys_a_cached_task_and_replays_an_output_as_attempt_zero() {
        let plan =
            Plan::new("gate", &[scan().cached(), resolve()], &["scan", "resolve"]).expect("valid");
        let (mut scheduler, decisions) = begun(&plan);
        let key = match &decisions[0] {
            Decision::Attempt {
                invocation: 0,
                attempt: 1,
                key: Some(key),
            } => key.clone(),
            other => panic!("a cached task was asked for without a key: {other:?}"),
        };
        assert_eq!(key.as_str().len(), 64);
        let hash = TaskOutputHash::of(&repo().bytes);
        let decisions = scheduler.replayed(0, key.clone(), hash.clone(), repo());
        match &decisions[0] {
            Decision::Recorded {
                invocation: 0,
                attempt,
            } => {
                assert!(attempt.replayed);
                assert_eq!(
                    (attempt.id().attempt, attempt.outcome()),
                    (0, &RunRecord::passed())
                );
                assert_eq!(
                    (attempt.output_hash(), attempt.task_cache_key()),
                    (Some(hash.clone()), Some(key.clone()))
                );
                assert!(attempt.began().is_some());
            }
            other => panic!("a replay was not recorded first: {other:?}"),
        }
        assert!(matches!(
            decisions[2],
            Decision::Attempt { invocation: 1, .. }
        ));
    }

    #[test]
    fn an_output_s_key_follows_the_task_and_its_inputs_and_nothing_else() {
        let plan = Plan::new("gate", &[scan()], &["scan"]).expect("valid");
        let (scheduler, _) = begun(&plan);
        let inputs = scheduler.read_inputs(0);
        let once = scheduler.key(&plan.tasks[0], &inputs);
        let twice = scheduler.key(&plan.tasks[0], &inputs);
        assert_eq!(once, twice);
        assert_eq!(once.as_str().len(), 64);
        let other = vec![ReadInput {
            value: Some(EncodedValue::encoded(&Root("other".to_string())).expect("encodes")),
            completion: None,
        }];
        assert_ne!(scheduler.key(&plan.tasks[0], &other), once);
        let (rebuilt, _) = RunScheduler::new(&plan, seeds(), Identity::of(b"8"), RunId::next());
        assert_ne!(
            rebuilt.key(&plan.tasks[0], &inputs),
            once,
            "another binary computes anew"
        );
    }

    #[test]
    fn a_read_fired_by_failing_or_finishing_is_handed_the_completion_not_only_a_value() {
        let mut store = RunStore::new();
        store.insert(
            TypeId::of::<A>(),
            EncodedValue::encoded(&()).expect("encodes"),
        );
        let mut completions = HashMap::new();
        finished::<A>(&mut completions, "p/a", RunRecord::passed());
        let inputs: Vec<ReadInput> = [
            Read::of::<A>(When::Passed),
            Read::of::<A>(When::Failed),
            Read::of::<A>(When::Finished),
        ]
        .iter()
        .map(|read| input_for(read, &store, &completions))
        .collect();
        assert!(inputs[0].value.is_some() && inputs[0].completion.is_none());
        assert!(inputs[1].value.is_none() && inputs[1].completion.is_some());
        let finishing = &inputs[2];
        assert!(finishing.value.is_some());
        let completion = finishing
            .completion
            .as_ref()
            .expect("a finishing read is handed the completion");
        assert_eq!(completion.attempt_id.path(), "p/a");
        assert_eq!(completion.outcome, RunRecord::passed());
    }

    #[test]
    fn read_inputs_come_in_the_reads_order() {
        let both = Task::new("both", |_: &mut TaskContext, _: (In<String>, In<u8>)| {
            Ok::<(), TaskFailure>(())
        });
        let plan = Plan::new("gate", &[both], &["both"]).expect("valid");
        let mut seeds = RunStore::new();
        seeds.put(&3u8).expect("encodes");
        seeds.put(&"x".to_string()).expect("encodes");
        let (scheduler, _) = RunScheduler::new(&plan, seeds, Identity::of(b"7"), RunId::next());
        let inputs = scheduler.read_inputs(0);
        let first: Option<String> = inputs[0]
            .value
            .as_ref()
            .and_then(|value| value.decoded().ok());
        let second: Option<u8> = inputs[1]
            .value
            .as_ref()
            .and_then(|value| value.decoded().ok());
        assert_eq!((first.as_deref(), second), (Some("x"), Some(3)));
    }

    #[test]
    fn a_reader_whose_every_read_fires_has_to_run() {
        let mut completions = HashMap::new();
        finished::<A>(&mut completions, "p/a", RunRecord::passed());
        finished::<B>(
            &mut completions,
            "p/b",
            RunRecord::failed("broke".to_string()),
        );
        let settled = outcome_from_producers(
            &[
                Read::of::<Seed>(When::Passed),
                Read::of::<A>(When::Passed),
                Read::of::<B>(When::Failed),
                Read::of::<B>(When::Finished),
            ],
            &completions,
        );
        assert_eq!(settled, None);
    }

    #[test]
    fn a_failure_read_of_a_task_that_passed_is_unneeded_and_names_that_task() {
        let mut completions = HashMap::new();
        finished::<A>(&mut completions, "p/a", RunRecord::passed());
        let settled = outcome_from_producers(&[Read::of::<A>(When::Failed)], &completions);
        assert_eq!(settled, Some(RunRecord::unneeded("p/a".to_string())));
    }

    #[test]
    fn a_value_read_of_a_branch_not_taken_is_unneeded_for_the_same_reason() {
        let mut completions = HashMap::new();
        finished::<A>(
            &mut completions,
            "p/a",
            RunRecord::unneeded("p/check".to_string()),
        );
        let settled = outcome_from_producers(&[Read::of::<A>(When::Passed)], &completions);
        assert_eq!(settled, Some(RunRecord::unneeded("p/check".to_string())));
    }

    #[test]
    fn a_skipped_producer_skips_its_reader_even_beside_a_branch_not_taken() {
        let mut completions = HashMap::new();
        finished::<A>(&mut completions, "p/a", RunRecord::passed());
        finished::<B>(
            &mut completions,
            "p/b",
            RunRecord::skipped("p/x".to_string()),
        );
        let settled = outcome_from_producers(
            &[Read::of::<A>(When::Failed), Read::of::<B>(When::Passed)],
            &completions,
        );
        assert_eq!(settled, Some(RunRecord::skipped("p/x".to_string())));
    }

    #[test]
    fn a_finishing_read_fires_whatever_the_producer_s_outcome() {
        let mut completions = HashMap::new();
        finished::<A>(
            &mut completions,
            "p/a",
            RunRecord::failed("broke".to_string()),
        );
        finished::<B>(
            &mut completions,
            "p/b",
            RunRecord::skipped("p/a".to_string()),
        );
        finished::<C>(
            &mut completions,
            "p/c",
            RunRecord::unneeded("p/a".to_string()),
        );
        let settled = outcome_from_producers(
            &[
                Read::of::<A>(When::Finished),
                Read::of::<B>(When::Finished),
                Read::of::<C>(When::Finished),
            ],
            &completions,
        );
        assert_eq!(settled, None);
    }

    #[test]
    fn a_timed_out_or_cancelled_producer_skips_its_value_readers_and_fires_its_failure_readers() {
        let mut completions = HashMap::new();
        finished::<A>(&mut completions, "p/a", RunRecord::timed_out());
        finished::<B>(&mut completions, "p/b", RunRecord::cancelled());
        assert_eq!(
            outcome_from_producers(&[Read::of::<A>(When::Passed)], &completions),
            Some(RunRecord::skipped("p/a".to_string()))
        );
        assert_eq!(
            outcome_from_producers(&[Read::of::<B>(When::Passed)], &completions),
            Some(RunRecord::skipped("p/b".to_string()))
        );
        assert_eq!(
            outcome_from_producers(
                &[Read::of::<A>(When::Failed), Read::of::<B>(When::Failed)],
                &completions
            ),
            None
        );
    }
}
