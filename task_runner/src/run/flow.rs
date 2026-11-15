use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::time::SystemTime;

use crate::job::id::JobId;
use crate::job::id::RunId;
use crate::plan::Plan;
use crate::store::Completion;
use crate::store::Part;
use crate::store::Product;
use crate::store::Store;
use crate::task::Task;
use crate::trace::outcome::Outcome;
use crate::trace::step::Step;

// What the flow asks of whoever drives it, in the order decided.
#[derive(Debug, PartialEq)]
pub enum Decision {
    // An attempt is wanted; with a key, the store may hold its product already.
    Attempt {
        at: usize,
        attempt: u32,
        key: Option<String>,
    },
    // The run was cancelled: whatever is out for this task is told to stop.
    Stop {
        at: usize,
    },
    // A record landed: an attempt's, a replay's, or one whose outcome followed
    // from its producers or from a cancel.
    Recorded {
        at: usize,
        step: Box<Step>,
    },
    // The task's outcome is final.
    Settled {
        at: usize,
        outcome: Outcome,
    },
}

// The run's bookkeeping, with nothing in it that waits, reads a file or
// starts a process: what waits on what, what has been asked for and not yet
// heard of, how many attempts each task has had, what each left, and the
// values held. Fed what happened, it answers with what should happen next.
pub struct Flow<'a> {
    plan: &'a Plan,
    store: Store,
    identity: u64,
    run: RunId,
    prefix: String,
    waits: Vec<usize>,
    tried: Vec<u32>,
    asked: Vec<bool>,
    left: usize,
    history: Vec<Vec<Step>>,
    cancelled: bool,
}

impl<'a> Flow<'a> {
    // Begins the run from the seeds: every task whose reads are all held is
    // decided at once.
    pub fn new(
        plan: &'a Plan,
        seeds: Store,
        identity: u64,
        run: RunId,
        prefix: &str,
    ) -> (Flow<'a>, Vec<Decision>) {
        let count = plan.tasks.len();
        let mut flow = Flow {
            plan,
            store: seeds,
            identity,
            run,
            prefix: prefix.to_string(),
            waits: plan.waits(),
            tried: vec![0; count],
            asked: vec![false; count],
            left: count,
            history: (0..count).map(|_| Vec::new()).collect(),
            cancelled: false,
        };
        let mut decisions = Vec::new();
        for at in 0..count {
            if flow.waits[at] == 0 {
                flow.ready(at, &mut decisions);
            }
        }
        (flow, decisions)
    }

    pub fn plan(&self) -> &'a Plan {
        self.plan
    }

    pub fn identity(&self) -> u64 {
        self.identity
    }

    pub fn job(&self, at: usize, attempt: u32) -> JobId {
        JobId::new(self.run, self.path(at), attempt)
    }

    fn path(&self, at: usize) -> String {
        format!(
            "{}{}/{}",
            self.prefix, self.plan.name, self.plan.tasks[at].name
        )
    }

    // The parts an attempt at the task is handed, in its reads' order.
    pub fn parts(&self, at: usize) -> Vec<Part> {
        self.store.pull(&self.plan.tasks[at].reads)
    }

    pub fn settled(&self) -> bool {
        self.left == 0
    }

    // Every record, task by task in plan order, and the values held.
    pub fn records(self) -> (Vec<Vec<Step>>, Store) {
        (self.history, self.store)
    }

    // The product was found in the store: the task is recorded as replayed,
    // attempt 0, and passes with it.
    pub fn replayed(&mut self, at: usize, product: Product) -> Vec<Decision> {
        let mut out = Vec::new();
        let mut step = Step::unrun(self.job(at, 0), &self.plan.tasks[at].name, Outcome::Ok);
        step.began = Some(SystemTime::now());
        step.replayed = true;
        self.asked[at] = false;
        self.record(at, step, &mut out);
        self.settle(at, Outcome::Ok, Some(product), &mut out);
        out
    }

    // An attempt is over: the task passed, earns another attempt, or is done.
    pub fn ended(&mut self, at: usize, step: Step, value: Option<Product>) -> Vec<Decision> {
        let mut out = Vec::new();
        let outcome = step.outcome.clone();
        if step.job.attempt > 0 {
            self.tried[at] = self.tried[at].max(step.job.attempt);
        }
        self.asked[at] = false;
        self.record(at, step, &mut out);
        let again = outcome.failed()
            && outcome != Outcome::Cancelled
            && !self.cancelled
            && self.tried[at] <= self.plan.tasks[at].retries;
        if outcome == Outcome::Ok {
            self.settle(at, outcome, value, &mut out);
        } else if again {
            self.ask(at, &mut out);
        } else {
            self.settle(at, outcome, None, &mut out);
        }
        out
    }

    // The run was cancelled: nothing earns an attempt after this, and every
    // attempt asked for and not yet heard of is to be stopped.
    pub fn cancel(&mut self) -> Vec<Decision> {
        self.cancelled = true;
        (0..self.plan.tasks.len())
            .filter(|&at| self.asked[at])
            .map(|at| Decision::Stop { at })
            .collect()
    }

    // An attempt asked for was never started: the task ends as cancelled.
    pub fn withdrawn(&mut self, at: usize) -> Vec<Decision> {
        let mut out = Vec::new();
        self.asked[at] = false;
        let step = Step::unrun(
            self.job(at, 0),
            &self.plan.tasks[at].name,
            Outcome::Cancelled,
        );
        self.record(at, step, &mut out);
        self.settle(at, Outcome::Cancelled, None, &mut out);
        out
    }

    // Every producer has finished: the task's outcome is already known and it
    // is recorded without an attempt, or an attempt is wanted.
    fn ready(&mut self, at: usize, out: &mut Vec<Decision>) {
        let task = &self.plan.tasks[at];
        if let Some(outcome) = self.store.settled(&task.reads) {
            let step = Step::unrun(self.job(at, 0), &task.name, outcome.clone());
            self.record(at, step, out);
            self.settle(at, outcome, None, out);
            return;
        }
        self.ask(at, out);
    }

    fn ask(&mut self, at: usize, out: &mut Vec<Decision>) {
        let task = &self.plan.tasks[at];
        let key = task
            .cached
            .then(|| self.key(task, &self.store.pull(&task.reads)));
        self.asked[at] = true;
        out.push(Decision::Attempt {
            at,
            attempt: self.tried[at] + 1,
            key,
        });
    }

    fn record(&mut self, at: usize, step: Step, out: &mut Vec<Decision>) {
        out.push(Decision::Recorded {
            at,
            step: Box::new(step.clone()),
        });
        self.history[at].push(step);
    }

    fn settle(
        &mut self,
        at: usize,
        outcome: Outcome,
        value: Option<Product>,
        out: &mut Vec<Decision>,
    ) {
        self.left -= 1;
        // A task leaving nothing has no type to be held under: its record is
        // all it leaves.
        if !self.plan.tasks[at].leaves_nothing() {
            let job = self.history[at]
                .last()
                .expect("a settled task has a record")
                .job
                .clone();
            self.store.finish(
                self.plan.tasks[at].produces.id,
                Completion {
                    job,
                    outcome: outcome.clone(),
                },
                value,
            );
        }
        out.push(Decision::Settled { at, outcome });
        let plan = self.plan;
        for &next in plan.dependents(at) {
            self.waits[next] -= 1;
            if self.waits[next] == 0 {
                self.ready(next, out);
            }
        }
    }

    // What a product is stored under: the worker binary, the task's name and
    // every input, so the same code on the same inputs finds it again.
    fn key(&self, task: &Task, parts: &[Part]) -> String {
        let mut hasher = DefaultHasher::new();
        self.identity.hash(&mut hasher);
        task.name.hash(&mut hasher);
        for (read, part) in task.reads.iter().zip(parts) {
            read.when.hash(&mut hasher);
            part.value
                .as_ref()
                .map(|product| &*product.bytes)
                .hash(&mut hasher);
            part.completion
                .as_ref()
                .map(|completion| format!("{:?}", completion.outcome))
                .hash(&mut hasher);
        }
        format!("{:016x}", hasher.finish())
    }
}

#[cfg(test)]
mod tests {
    use std::any::TypeId;

    use serde::Deserialize;
    use serde::Serialize;

    use super::Decision;
    use super::Flow;
    use crate::job::id::RunId;
    use crate::plan::Plan;
    use crate::pull::Failed;
    use crate::pull::In;
    use crate::scope::Scope;
    use crate::store::Part;
    use crate::store::Product;
    use crate::store::Store;
    use crate::task::Task;
    use crate::trace::outcome::Failure;
    use crate::trace::outcome::Outcome;
    use crate::trace::step::Step;

    #[derive(Serialize, Deserialize)]
    struct Root(String);
    #[derive(Serialize, Deserialize)]
    struct Repo;
    #[derive(Serialize, Deserialize)]
    struct Graph;
    #[derive(Serialize, Deserialize)]
    struct Fixed;

    fn scan() -> Task {
        Task::new("scan", |_: &mut Scope, _: In<Root>| {
            Ok::<Repo, Failure>(Repo)
        })
    }

    fn resolve() -> Task {
        Task::new("resolve", |_: &mut Scope, _: In<Repo>| {
            Ok::<Graph, Failure>(Graph)
        })
    }

    fn fix() -> Task {
        Task::new("fix", |_: &mut Scope, _: Failed<Repo>| {
            Ok::<Fixed, Failure>(Fixed)
        })
    }

    fn seeds() -> Store {
        let mut store = Store::new();
        store.put(&Root("repo".to_string())).expect("encodes");
        store
    }

    fn begun(plan: &Plan) -> (Flow<'_>, Vec<Decision>) {
        Flow::new(plan, seeds(), 7, RunId::next(), "")
    }

    // The record a process attempt leaves, as the driver hands it back.
    fn attempt(flow: &Flow<'_>, at: usize, attempt: u32, outcome: Outcome) -> Step {
        Step::unrun(flow.job(at, attempt), &flow.plan().tasks[at].name, outcome)
    }

    fn passed(flow: &Flow<'_>, at: usize, attempt: u32) -> Step {
        self::attempt(flow, at, attempt, Outcome::Ok)
    }

    fn failed(flow: &Flow<'_>, at: usize, attempt: u32) -> Step {
        self::attempt(flow, at, attempt, Outcome::Failed("no".to_string()))
    }

    fn repo() -> Product {
        Product::encoded(&Repo).expect("encodes")
    }

    fn nothing(name: &str) -> Task {
        Task::new(name, |_: &mut Scope, (): ()| Ok::<(), Failure>(()))
    }

    #[test]
    fn a_flow_keeps_no_value_for_a_task_leaving_nothing() {
        let known = [nothing("one"), nothing("two")];
        let plan = Plan::new("gate", &known, &["one", "two"]).expect("valid");
        let (mut flow, _) = begun(&plan);
        let step = passed(&flow, 0, 1);
        flow.ended(0, step, Some(Product::encoded(&()).expect("encodes")));
        let step = failed(&flow, 1, 1);
        flow.ended(1, step, None);
        assert!(flow.settled());
        let (_, store) = flow.records();
        assert!(!store.has(TypeId::of::<()>()));
    }

    #[test]
    fn a_flow_asks_first_for_the_tasks_whose_reads_are_all_held() {
        let plan = Plan::new("gate", &[scan(), resolve()], &["scan", "resolve"]).expect("valid");
        let (flow, decisions) = begun(&plan);
        assert_eq!(
            decisions,
            [Decision::Attempt {
                at: 0,
                attempt: 1,
                key: None
            }]
        );
        assert!(!flow.settled());
    }

    #[test]
    fn a_flow_asks_for_a_reader_the_moment_its_producer_passes() {
        let plan = Plan::new("gate", &[scan(), resolve()], &["scan", "resolve"]).expect("valid");
        let (mut flow, _) = begun(&plan);
        let step = passed(&flow, 0, 1);
        let decisions = flow.ended(0, step.clone(), Some(repo()));
        assert_eq!(
            decisions,
            [
                Decision::Recorded {
                    at: 0,
                    step: Box::new(step)
                },
                Decision::Settled {
                    at: 0,
                    outcome: Outcome::Ok
                },
                Decision::Attempt {
                    at: 1,
                    attempt: 1,
                    key: None
                },
            ]
        );
        let decisions = flow.ended(
            1,
            passed(&flow, 1, 1),
            Some(Product::encoded(&Graph).expect("encodes")),
        );
        assert!(matches!(
            decisions.last(),
            Some(Decision::Settled { at: 1, .. })
        ));
        assert!(flow.settled());
        let (history, store) = flow.records();
        assert_eq!(history.iter().map(Vec::len).collect::<Vec<_>>(), [1, 1]);
        assert!(store.get::<Graph>().is_some());
    }

    #[test]
    fn a_flow_skips_the_readers_of_a_failed_task_and_asks_for_its_failure_readers() {
        let plan = Plan::new(
            "gate",
            &[scan(), resolve(), fix()],
            &["scan", "resolve", "fix"],
        )
        .expect("valid");
        let (mut flow, _) = begun(&plan);
        let decisions = flow.ended(0, failed(&flow, 0, 1), None);
        let skipped = Step::unrun(
            flow.job(1, 0),
            "resolve",
            Outcome::Skipped("gate/scan".to_string()),
        );
        assert_eq!(
            decisions[1..],
            [
                Decision::Settled {
                    at: 0,
                    outcome: Outcome::Failed("no".to_string())
                },
                Decision::Recorded {
                    at: 1,
                    step: Box::new(skipped)
                },
                Decision::Settled {
                    at: 1,
                    outcome: Outcome::Skipped("gate/scan".to_string())
                },
                Decision::Attempt {
                    at: 2,
                    attempt: 1,
                    key: None
                },
            ]
        );
    }

    #[test]
    fn a_flow_asks_again_after_a_failure_until_the_retries_are_spent() {
        let plan = Plan::new("gate", &[scan().retries(1)], &["scan"]).expect("valid");
        let (mut flow, _) = begun(&plan);
        let decisions = flow.ended(0, failed(&flow, 0, 1), None);
        assert_eq!(
            decisions[1],
            Decision::Attempt {
                at: 0,
                attempt: 2,
                key: None
            }
        );
        let decisions = flow.ended(0, failed(&flow, 0, 2), None);
        assert_eq!(
            decisions[1],
            Decision::Settled {
                at: 0,
                outcome: Outcome::Failed("no".to_string())
            }
        );
        assert!(flow.settled());
    }

    #[test]
    fn a_flow_cancelled_stops_what_was_asked_and_a_withdrawn_attempt_ends_as_attempt_zero() {
        let plan = Plan::new("gate", &[scan(), fix().retries(2)], &["scan", "fix"]).expect("valid");
        let (mut flow, _) = begun(&plan);
        assert_eq!(flow.cancel(), [Decision::Stop { at: 0 }]);
        let decisions = flow.withdrawn(0);
        let cancelled = Step::unrun(flow.job(0, 0), "scan", Outcome::Cancelled);
        assert_eq!(
            decisions,
            [
                Decision::Recorded {
                    at: 0,
                    step: Box::new(cancelled)
                },
                Decision::Settled {
                    at: 0,
                    outcome: Outcome::Cancelled
                },
                Decision::Attempt {
                    at: 1,
                    attempt: 1,
                    key: None
                },
            ]
        );
        // What the cancel unlocked still runs, but a failure earns no retry now.
        let decisions = flow.ended(1, failed(&flow, 1, 1), None);
        assert_eq!(
            decisions[1],
            Decision::Settled {
                at: 1,
                outcome: Outcome::Failed("no".to_string())
            }
        );
        assert!(flow.settled());
    }

    #[test]
    fn a_flow_keys_a_cached_task_and_replays_a_product_as_attempt_zero() {
        let plan =
            Plan::new("gate", &[scan().cached(), resolve()], &["scan", "resolve"]).expect("valid");
        let (mut flow, decisions) = begun(&plan);
        let key = match &decisions[0] {
            Decision::Attempt {
                at: 0,
                attempt: 1,
                key: Some(key),
            } => key.clone(),
            other => panic!("a cached task was asked for without a key: {other:?}"),
        };
        assert_eq!(key.len(), 16);
        let decisions = flow.replayed(0, repo());
        match &decisions[0] {
            Decision::Recorded { at: 0, step } => {
                assert!(step.replayed);
                assert_eq!((step.job.attempt, &step.outcome), (0, &Outcome::Ok));
                assert!(step.began.is_some());
            }
            other => panic!("a replay was not recorded first: {other:?}"),
        }
        assert!(matches!(decisions[2], Decision::Attempt { at: 1, .. }));
    }

    #[test]
    fn a_product_s_key_follows_the_task_and_its_inputs_and_nothing_else() {
        let plan = Plan::new("gate", &[scan()], &["scan"]).expect("valid");
        let (flow, _) = begun(&plan);
        let parts = flow.parts(0);
        let once = flow.key(&plan.tasks[0], &parts);
        let twice = flow.key(&plan.tasks[0], &parts);
        assert_eq!(once, twice);
        assert_eq!(once.len(), 16);
        let other = vec![Part {
            value: Some(Product::encoded(&Root("other".to_string())).expect("encodes")),
            completion: None,
        }];
        assert_ne!(flow.key(&plan.tasks[0], &other), once);
        let (rebuilt, _) = Flow::new(&plan, seeds(), 8, RunId::next(), "");
        assert_ne!(
            rebuilt.key(&plan.tasks[0], &parts),
            once,
            "another binary computes anew"
        );
    }
}
