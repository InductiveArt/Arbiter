use std::any::TypeId;
use std::collections::HashMap;
use std::collections::HashSet;

use crate::wip::store::Need;
use crate::wip::store::When;
use crate::wip::task::Task;

#[derive(Debug, PartialEq)]
pub enum Invalid {
    // A name none of the known tasks bears: the worker could never find it.
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
}

// Tasks selected by name from those the binary knows, wired by type into
// something that can run: each type has one producer, no task waits on itself
// through others, and names are unique so ids are too.
pub struct Plan {
    pub name: String,
    pub tasks: Vec<Task>,
    // For each task, the tasks reading what it produces.
    dependents: Vec<Vec<usize>>,
    // For each task, how many of its reads a task here produces.
    waits: Vec<usize>,
}

impl Plan {
    pub fn new(name: &str, known: &[Task], names: &[&str]) -> Result<Plan, Invalid> {
        let mut tasks: Vec<Task> = Vec::with_capacity(names.len());
        for &wanted in names {
            match known.iter().find(|task| task.name == wanted) {
                Some(task) => tasks.push(task.clone()),
                None => return Err(Invalid::Unknown(wanted.to_string())),
            }
        }
        let mut seen: HashSet<&str> = HashSet::new();
        for task in &tasks {
            if !seen.insert(task.name.as_str()) {
                return Err(Invalid::DuplicateName(task.name.clone()));
            }
        }
        let mut producer: HashMap<TypeId, usize> = HashMap::new();
        for (at, task) in tasks.iter().enumerate() {
            if let Some(&first) = producer.get(&task.produces.id) {
                return Err(Invalid::TwoProducers {
                    of: task.produces.name,
                    first: tasks[first].name.clone(),
                    second: task.name.clone(),
                });
            }
            producer.insert(task.produces.id, at);
        }
        let mut dependents = vec![Vec::new(); tasks.len()];
        let mut waits = vec![0; tasks.len()];
        for (at, task) in tasks.iter().enumerate() {
            for read in &task.reads {
                match producer.get(&read.need.id) {
                    Some(&from) => {
                        dependents[from].push(at);
                        waits[at] += 1;
                    }
                    None if read.when != When::Passed => {
                        return Err(Invalid::NoProducer {
                            of: read.need.name,
                            reader: task.name.clone(),
                        });
                    }
                    None => {}
                }
            }
        }
        // Everything orders, or whatever remains waits on itself through others.
        let mut remaining = waits.clone();
        let mut ready: Vec<usize> = (0..tasks.len()).filter(|&at| remaining[at] == 0).collect();
        let mut ordered = 0;
        while let Some(at) = ready.pop() {
            ordered += 1;
            for &next in &dependents[at] {
                remaining[next] -= 1;
                if remaining[next] == 0 {
                    ready.push(next);
                }
            }
        }
        if ordered < tasks.len() {
            let stuck = (0..tasks.len())
                .filter(|&at| remaining[at] > 0)
                .map(|at| tasks[at].name.clone())
                .collect();
            return Err(Invalid::Cycle(stuck));
        }
        Ok(Plan {
            name: name.to_string(),
            tasks,
            dependents,
            waits,
        })
    }

    // The types read that no task here produces: what a run must be seeded with.
    pub fn required(&self) -> Vec<Need> {
        let produced: HashSet<TypeId> = self.tasks.iter().map(|task| task.produces.id).collect();
        let mut needs: Vec<Need> = Vec::new();
        for task in &self.tasks {
            for read in &task.reads {
                if !produced.contains(&read.need.id) && !needs.contains(&read.need) {
                    needs.push(read.need);
                }
            }
        }
        needs
    }

    pub fn dependents(&self, at: usize) -> &[usize] {
        &self.dependents[at]
    }

    pub fn waits(&self) -> Vec<usize> {
        self.waits.clone()
    }

    pub fn position(&self, name: &str) -> Option<usize> {
        self.tasks.iter().position(|task| task.name == name)
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use serde::Serialize;

    use super::Invalid;
    use super::Plan;
    use crate::wip::store::Need;
    use crate::wip::task::Failed;
    use crate::wip::task::In;
    use crate::wip::task::Task;
    use crate::wip::trace::Failure;
    use crate::wip::trace::Scope;

    #[derive(Serialize, Deserialize)]
    struct A;
    #[derive(Serialize, Deserialize)]
    struct B;

    fn a() -> Task {
        Task::new("a", |_: &mut Scope, (): ()| Ok::<A, Failure>(A))
    }

    fn b_from_a() -> Task {
        Task::new("b", |_: &mut Scope, _: In<A>| Ok::<B, Failure>(B))
    }

    fn b_when_a_fails() -> Task {
        Task::new("b", |_: &mut Scope, _: Failed<A>| Ok::<B, Failure>(B))
    }

    #[test]
    fn a_plan_refuses_a_task_the_registry_does_not_know() {
        let refused = Plan::new("p", &[a()], &["a", "c"]).err();
        assert_eq!(refused, Some(Invalid::Unknown("c".to_string())));
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
        assert_eq!(refused, Some(Invalid::DuplicateName("a".to_string())));
    }

    #[test]
    fn a_plan_rejects_a_type_two_tasks_produce() {
        let twin = Task::new("a2", |_: &mut Scope, (): ()| Ok::<A, Failure>(A));
        let refused = Plan::new("p", &[a(), twin], &["a", "a2"]).err();
        match refused {
            Some(Invalid::TwoProducers { of, first, second }) => {
                assert!(of.ends_with("::A"), "{of} names the type");
                assert_eq!((first.as_str(), second.as_str()), ("a", "a2"));
            }
            other => panic!("two producers were not refused: {other:?}"),
        }
    }

    #[test]
    fn a_plan_rejects_a_cycle_in_its_data() {
        let a_from_b = Task::new("a", |_: &mut Scope, _: In<B>| Ok::<A, Failure>(A));
        let refused = Plan::new("p", &[a_from_b, b_from_a()], &["a", "b"]).err();
        assert_eq!(
            refused,
            Some(Invalid::Cycle(vec!["a".to_string(), "b".to_string()]))
        );
    }

    #[test]
    fn a_plan_rejects_a_read_fired_by_failing_or_finishing_of_a_type_no_task_produces() {
        let refused = Plan::new("p", &[b_when_a_fails()], &["b"]).err();
        match refused {
            Some(Invalid::NoProducer { of, reader }) => {
                assert!(of.ends_with("::A"), "{of} names the type");
                assert_eq!(reader, "b");
            }
            other => panic!("a conditional read of a seed was not refused: {other:?}"),
        }
    }

    #[test]
    fn a_plan_names_the_types_it_must_be_seeded_with() {
        let plan = Plan::new("p", &[b_from_a()], &["b"]).expect("one task reading one type");
        assert_eq!(plan.required(), [Need::of::<A>()]);
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

    #[test]
    fn a_plan_finds_a_task_s_position_by_name() {
        let plan = Plan::new("p", &[a(), b_from_a()], &["a", "b"]).expect("a chain");
        assert_eq!(plan.position("b"), Some(1));
        assert_eq!(plan.position("c"), None);
    }
}
