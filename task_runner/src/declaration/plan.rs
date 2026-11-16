use std::any::TypeId;
use std::collections::HashMap;
use std::collections::HashSet;

use crate::declaration::fan_out::FanOut;
use crate::declaration::read::Read;
use crate::declaration::read::When;
use crate::declaration::task::Task;
use crate::value::value_type::ValueType;

#[derive(Debug, PartialEq)]
pub enum InvalidPlan {
    // A name none of the known tasks bears: the task process could never find it.
    Unknown(String),
    DuplicateName(String),
    TwoProducers {
        of: &'static str,
        first: String,
        second: String,
    },
    // A read fired by failing or finishing waits on a task's completion, which
    // a seed never has.
    NoProducer {
        of: &'static str,
        reader: String,
    },
    Cycle(Vec<String>),
    // A fan-out the plan cannot hold: of no task here, of one task twice, or
    // of a task that does not read the item once as passed or does not leave
    // the output named.
    FanOut {
        task: String,
        reason: &'static str,
    },
}

// Tasks selected by name from those the binary knows, wired by type into
// something that can run: each type has one producer, any number of tasks
// leave nothing, no task waits on itself through others, and names are unique
// so ids are too. A fanned task reads, in the run, the list in place of its
// item, and leaves the gathered list in place of what one run leaves.
pub struct Plan {
    pub name: String,
    pub tasks: Vec<Task>,
    fan_outs: Vec<Option<FanOut>>,
    // For each task, the tasks reading what it produces.
    dependents: Vec<Vec<usize>>,
    // For each task, how many tasks here it waits on.
    waits: Vec<usize>,
}

impl Plan {
    pub fn new(name: &str, known: &[Task], names: &[&str]) -> Result<Plan, InvalidPlan> {
        Plan::fanned(name, known, names, &[])
    }

    pub fn fanned(
        name: &str,
        known: &[Task],
        names: &[&str],
        fan_outs: &[FanOut],
    ) -> Result<Plan, InvalidPlan> {
        let mut tasks: Vec<Task> = Vec::with_capacity(names.len());
        for &wanted in names {
            match known.iter().find(|task| task.name == wanted) {
                Some(task) => tasks.push(task.clone()),
                None => return Err(InvalidPlan::Unknown(wanted.to_string())),
            }
        }
        let mut seen: HashSet<&str> = HashSet::new();
        for task in &tasks {
            if !seen.insert(task.name.as_str()) {
                return Err(InvalidPlan::DuplicateName(task.name.clone()));
            }
        }
        let fanned = fanned(&tasks, fan_outs)?;
        let mut producer: HashMap<TypeId, usize> = HashMap::new();
        for (at, task) in tasks.iter().enumerate() {
            if task.leaves_nothing() {
                continue;
            }
            let produces = run_produces(task, fanned[at].as_ref());
            if let Some(&first) = producer.get(&produces.id) {
                return Err(InvalidPlan::TwoProducers {
                    of: produces.name,
                    first: tasks[first].name.clone(),
                    second: task.name.clone(),
                });
            }
            producer.insert(produces.id, at);
        }
        let mut dependents = vec![Vec::new(); tasks.len()];
        let mut waits = vec![0; tasks.len()];
        for (at, task) in tasks.iter().enumerate() {
            for read in run_reads(task, fanned[at].as_ref()) {
                match producer.get(&read.value_type.id) {
                    Some(&from) => {
                        dependents[from].push(at);
                        waits[at] += 1;
                    }
                    None if read.when != When::Passed => {
                        return Err(InvalidPlan::NoProducer {
                            of: read.value_type.name,
                            reader: task.name.clone(),
                        });
                    }
                    None => {}
                }
            }
        }
        let plan = Plan {
            name: name.to_string(),
            tasks,
            fan_outs: fanned,
            dependents,
            waits,
        };
        plan.orders()?;
        Ok(plan)
    }

    // Everything orders, or whatever remains waits on itself through others.
    fn orders(&self) -> Result<(), InvalidPlan> {
        let mut remaining = self.waits.clone();
        let mut ready: Vec<usize> = (0..self.tasks.len())
            .filter(|&at| remaining[at] == 0)
            .collect();
        let mut ordered = 0;
        while let Some(at) = ready.pop() {
            ordered += 1;
            for &next in &self.dependents[at] {
                remaining[next] -= 1;
                if remaining[next] == 0 {
                    ready.push(next);
                }
            }
        }
        if ordered < self.tasks.len() {
            let stuck = (0..self.tasks.len())
                .filter(|&at| remaining[at] > 0)
                .map(|at| self.tasks[at].name.clone())
                .collect();
            return Err(InvalidPlan::Cycle(stuck));
        }
        Ok(())
    }

    pub fn fan_out(&self, at: usize) -> Option<&FanOut> {
        self.fan_outs[at].as_ref()
    }

    pub fn reads(&self, at: usize) -> Vec<Read> {
        run_reads(&self.tasks[at], self.fan_out(at))
    }

    pub fn produces(&self, at: usize) -> ValueType {
        run_produces(&self.tasks[at], self.fan_out(at))
    }

    // The types read that no task here produces: what a run must be seeded with.
    pub fn required(&self) -> Vec<ValueType> {
        let produced: HashSet<TypeId> = (0..self.tasks.len())
            .filter(|&at| !self.tasks[at].leaves_nothing())
            .map(|at| self.produces(at).id)
            .collect();
        let mut value_types: Vec<ValueType> = Vec::new();
        for at in 0..self.tasks.len() {
            for read in self.reads(at) {
                if !produced.contains(&read.value_type.id)
                    && !value_types.contains(&read.value_type)
                {
                    value_types.push(read.value_type);
                }
            }
        }
        value_types
    }

    pub(crate) fn dependents(&self, at: usize) -> &[usize] {
        &self.dependents[at]
    }

    pub(crate) fn waits(&self) -> Vec<usize> {
        self.waits.clone()
    }
}

// Each task's fan-out, checked against the task it names.
fn fanned(tasks: &[Task], fan_outs: &[FanOut]) -> Result<Vec<Option<FanOut>>, InvalidPlan> {
    let mut fanned: Vec<Option<FanOut>> = vec![None; tasks.len()];
    for fan_out in fan_outs {
        let refused = |reason| InvalidPlan::FanOut {
            task: fan_out.task.clone(),
            reason,
        };
        let Some(at) = tasks.iter().position(|task| task.name == fan_out.task) else {
            return Err(refused("no task of the plan bears the name"));
        };
        let task = &tasks[at];
        let items: Vec<&Read> = task
            .reads
            .iter()
            .filter(|read| read.value_type == fan_out.item)
            .collect();
        if fanned[at].is_some() {
            return Err(refused("the task is fanned out twice"));
        }
        if items.len() != 1 || items[0].when != When::Passed {
            return Err(refused("the task does not read the item once, as passed"));
        }
        if task.produces != fan_out.output || task.leaves_nothing() {
            return Err(refused("the task does not leave the output named"));
        }
        fanned[at] = Some(fan_out.clone());
    }
    Ok(fanned)
}

// What a task reads in the run: the list in place of the item it is fanned
// out over.
fn run_reads(task: &Task, fan_out: Option<&FanOut>) -> Vec<Read> {
    task.reads
        .iter()
        .map(|read| match fan_out {
            Some(fan_out) if read.value_type == fan_out.item => Read {
                value_type: fan_out.list,
                when: read.when,
            },
            _ => *read,
        })
        .collect()
}

// What a task leaves in the run: the gathered list when it is fanned out.
fn run_produces(task: &Task, fan_out: Option<&FanOut>) -> ValueType {
    fan_out.map_or(task.produces, |fan_out| fan_out.gathered)
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use serde::Serialize;

    use super::InvalidPlan;
    use super::Plan;
    use crate::context::task_context::TaskContext;
    use crate::declaration::fan_out::FanOut;
    use crate::declaration::pull::Failed;
    use crate::declaration::pull::In;
    use crate::declaration::task::Task;
    use crate::record::task_failure::TaskFailure;
    use crate::value::value_type::ValueType;

    #[derive(Serialize, Deserialize)]
    struct A;
    #[derive(Serialize, Deserialize)]
    struct B;

    fn a() -> Task {
        Task::new("a", |_: &mut TaskContext, (): ()| Ok::<A, TaskFailure>(A))
    }

    fn b_from_a() -> Task {
        Task::new("b", |_: &mut TaskContext, _: In<A>| Ok::<B, TaskFailure>(B))
    }

    fn b_when_a_fails() -> Task {
        Task::new("b", |_: &mut TaskContext, _: Failed<A>| {
            Ok::<B, TaskFailure>(B)
        })
    }

    #[test]
    fn a_plan_refuses_a_name_none_of_the_known_tasks_bears() {
        let refused = Plan::new("p", &[a()], &["a", "c"]).err();
        assert_eq!(refused, Some(InvalidPlan::Unknown("c".to_string())));
    }

    #[test]
    fn a_plan_holds_its_tasks_in_the_order_named() {
        let plan = Plan::new("p", &[a(), b_from_a()], &["b", "a"]).expect("a chain");
        let names: Vec<&str> = plan.tasks.iter().map(|task| task.name.as_str()).collect();
        assert_eq!(names, ["b", "a"]);
    }

    #[test]
    fn a_plan_rejects_two_tasks_of_one_name() {
        let refused = Plan::new("p", &[a()], &["a", "a"]).err();
        assert_eq!(refused, Some(InvalidPlan::DuplicateName("a".to_string())));
    }

    #[test]
    fn a_plan_rejects_a_type_two_tasks_produce() {
        let twin = Task::new("a2", |_: &mut TaskContext, (): ()| Ok::<A, TaskFailure>(A));
        let refused = Plan::new("p", &[a(), twin], &["a", "a2"]).err();
        match refused {
            Some(InvalidPlan::TwoProducers { of, first, second }) => {
                assert!(of.ends_with("::A"), "{of} names the type");
                assert_eq!((first.as_str(), second.as_str()), ("a", "a2"));
            }
            other => panic!("two producers were not refused: {other:?}"),
        }
    }

    fn nothing(name: &str) -> Task {
        Task::new(
            name,
            |_: &mut TaskContext, (): ()| Ok::<(), TaskFailure>(()),
        )
    }

    #[test]
    fn a_plan_takes_any_number_of_tasks_leaving_nothing() {
        let plan = Plan::new("p", &[nothing("one"), nothing("two")], &["one", "two"]);
        assert!(plan.is_ok(), "nothing is no type two tasks both produce");
    }

    #[test]
    fn a_read_of_nothing_is_not_answered_by_a_task_leaving_nothing() {
        let reader = Task::new("reader", |_: &mut TaskContext, _: In<()>| {
            Ok::<A, TaskFailure>(A)
        });
        let plan = Plan::new("p", &[nothing("one"), reader], &["one", "reader"]).expect("valid");
        assert_eq!(plan.waits(), [0, 0]);
        assert_eq!(plan.required(), [ValueType::of::<()>()]);
    }

    #[test]
    fn a_plan_rejects_a_cycle_in_its_data() {
        let a_from_b = Task::new("a", |_: &mut TaskContext, _: In<B>| Ok::<A, TaskFailure>(A));
        let refused = Plan::new("p", &[a_from_b, b_from_a()], &["a", "b"]).err();
        assert_eq!(
            refused,
            Some(InvalidPlan::Cycle(vec!["a".to_string(), "b".to_string()]))
        );
    }

    #[test]
    fn a_plan_rejects_a_read_fired_by_failing_or_finishing_of_a_type_no_task_produces() {
        let refused = Plan::new("p", &[b_when_a_fails()], &["b"]).err();
        match refused {
            Some(InvalidPlan::NoProducer { of, reader }) => {
                assert!(of.ends_with("::A"), "{of} names the type");
                assert_eq!(reader, "b");
            }
            other => panic!("a conditional read of a seed was not refused: {other:?}"),
        }
    }

    #[test]
    fn a_plan_names_the_types_it_must_be_seeded_with() {
        let plan = Plan::new("p", &[b_from_a()], &["b"]).expect("one task reading one type");
        assert_eq!(plan.required(), [ValueType::of::<A>()]);
    }

    #[test]
    fn a_plan_wires_a_reader_behind_its_producer() {
        let plan = Plan::new("p", &[a(), b_from_a()], &["a", "b"]).expect("a chain");
        assert_eq!(plan.waits(), [0, 1]);
        assert_eq!(plan.dependents(0), [1]);
        assert!(plan.required().is_empty());
    }

    #[test]
    fn a_plan_wires_a_reader_of_a_failure_behind_its_producer_like_any_reader() {
        let plan = Plan::new("p", &[a(), b_when_a_fails()], &["a", "b"]).expect("a branch");
        assert_eq!(plan.waits(), [0, 1]);
        assert_eq!(plan.dependents(0), [1]);
        assert!(plan.required().is_empty());
    }

    fn list() -> Task {
        Task::new("list", |_: &mut TaskContext, (): ()| {
            Ok::<Vec<A>, TaskFailure>(vec![A, A])
        })
    }

    fn gather() -> Task {
        Task::new("gather", |_: &mut TaskContext, _: In<Vec<B>>| {
            Ok::<(), TaskFailure>(())
        })
    }

    #[test]
    fn a_fanned_task_waits_on_the_list_and_its_gathered_outputs_are_read_as_a_list() {
        let plan = Plan::fanned(
            "p",
            &[list(), b_from_a(), gather()],
            &["list", "b", "gather"],
            &[FanOut::of::<A, B>("b", |_| "a".to_string())],
        )
        .expect("a fanned chain");
        assert_eq!(plan.waits(), [0, 1, 1]);
        assert_eq!(
            (plan.dependents(0), plan.dependents(1)),
            (&[1][..], &[2][..])
        );
        assert_eq!(plan.produces(1), ValueType::of::<Vec<B>>());
        assert!(plan.required().is_empty());
    }

    #[test]
    fn a_fanned_task_over_a_list_no_task_leaves_needs_the_list_seeded() {
        let plan = Plan::fanned(
            "p",
            &[b_from_a()],
            &["b"],
            &[FanOut::of::<A, B>("b", |_| "a".to_string())],
        )
        .expect("valid");
        assert_eq!(plan.required(), [ValueType::of::<Vec<A>>()]);
    }

    #[test]
    fn a_fan_out_the_task_cannot_take_is_refused_with_its_reason() {
        let refused = |fan_out: FanOut| match Plan::fanned(
            "p",
            &[a(), b_from_a()],
            &["a", "b"],
            &[fan_out],
        ) {
            Err(InvalidPlan::FanOut { reason, .. }) => reason,
            other => panic!("the fan-out was taken: {:?}", other.err()),
        };
        assert_eq!(
            refused(FanOut::of::<A, B>("c", |_| "a".to_string())),
            "no task of the plan bears the name"
        );
        assert_eq!(
            refused(FanOut::of::<B, B>("b", |_| "b".to_string())),
            "the task does not read the item once, as passed"
        );
        assert_eq!(
            refused(FanOut::of::<A, A>("b", |_| "a".to_string())),
            "the task does not leave the output named"
        );
    }
}
