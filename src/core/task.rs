use std::process::ExitCode;
use std::time::Instant;

// A named unit of work: it parses its own arguments, does the work and reports.
pub struct Task {
    pub name: &'static str,
    pub line: &'static str,
    pub run: fn(&[String]) -> ExitCode,
}

// A named judgment the gate runs under its judge task: it reads the resolved
// graph and yields the findings that break it. The gate gathers every law's
// findings the way a parent task gathers its children.
pub struct Law {
    pub task: &'static str,
    pub run: fn(&crate::core::graph::Graph) -> Vec<crate::core::pack::Finding>,
}

// How a task ended. A task ends Failed when the work itself could not stand.
pub enum Outcome {
    Ok,
    Failed,
}

// One task in the run tree: an operation that began, ran and ended. A leaf
// carries its own measured cost; an orchestrator carries the sum of its
// children, so no cost is counted twice.
pub struct Node {
    pub op: String,
    pub outcome: Outcome,
    pub seconds: f64,
    pub evidence: Vec<String>,
    pub children: Vec<Node>,
}

// What a task body hands back: its typed value, how it ended, and any evidence
// it leaves on its node.
pub struct Done<T> {
    pub value: T,
    pub outcome: Outcome,
    pub evidence: Vec<String>,
}

impl<T> Done<T> {
    pub fn ok(value: T) -> Self {
        Done {
            value,
            outcome: Outcome::Ok,
            evidence: Vec::new(),
        }
    }

    pub fn ended(value: T, outcome: Outcome) -> Self {
        Done {
            value,
            outcome,
            evidence: Vec::new(),
        }
    }

    // One line of evidence recorded on this task's node.
    pub fn noting(mut self, evidence: impl Into<String>) -> Self {
        self.evidence.push(evidence.into());
        self
    }
}

// The run tree, built as tasks begin and end. Thread it through the work: a
// task calls run() for each child it spawns, and the nesting is the tree.
#[derive(Default)]
pub struct Journal {
    open: Vec<Frame>,
    done: Vec<Node>,
}

struct Frame {
    op: String,
    start: Instant,
    children: Vec<Node>,
}

impl Journal {
    pub fn new() -> Self {
        Journal::default()
    }

    // Run one task under the task now open, or at the root when none is. The
    // body does the work and may call run() again for its own children; its
    // cost is the body's elapsed time, or the sum of its children when it has
    // any.
    pub fn run<T>(&mut self, op: &str, body: impl FnOnce(&mut Journal) -> Done<T>) -> T {
        self.open.push(Frame {
            op: op.to_string(),
            start: Instant::now(),
            children: Vec::new(),
        });
        let done = body(self);
        let frame = self.open.pop().expect("run opened this frame");
        let seconds = if frame.children.is_empty() {
            frame.start.elapsed().as_secs_f64()
        } else {
            frame.children.iter().map(|child| child.seconds).sum()
        };
        let node = Node {
            op: frame.op,
            outcome: done.outcome,
            seconds,
            evidence: done.evidence,
            children: frame.children,
        };
        self.place(node);
        done.value
    }

    // Record a leaf whose cost was measured elsewhere, by whatever performed it.
    pub fn noted(&mut self, op: &str, seconds: f64, evidence: Vec<String>) {
        self.place(Node {
            op: op.to_string(),
            outcome: Outcome::Ok,
            seconds,
            evidence,
            children: Vec::new(),
        });
    }

    fn place(&mut self, node: Node) {
        match self.open.last_mut() {
            Some(parent) => parent.children.push(node),
            None => self.done.push(node),
        }
    }

    // The completed forest: each task run at the root, with its subtree.
    pub fn tree(self) -> Vec<Node> {
        self.done
    }
}

#[cfg(test)]
mod tests {
    use super::Done;
    use super::Journal;

    #[test]
    fn a_task_tree_nests_children_under_the_task_that_spawned_them() {
        let mut journal = Journal::new();
        let sum = journal.run("judge", |j| {
            let a = j.run("graph", |_| Done::ok(2));
            let b = j.run("plans", |_| Done::ok(3));
            Done::ok(a + b).noting("5 findings")
        });
        assert_eq!(sum, 5);
        let tree = journal.tree();
        assert_eq!(tree.len(), 1, "one task ran at the root");
        let judge = &tree[0];
        assert_eq!(judge.op, "judge");
        assert_eq!(judge.evidence, ["5 findings"]);
        let names: Vec<&str> = judge.children.iter().map(|c| c.op.as_str()).collect();
        assert_eq!(
            names,
            ["graph", "plans"],
            "children sit in the order they ran"
        );
    }

    #[test]
    fn a_noted_leaf_sits_beside_the_tasks_that_run() {
        let mut journal = Journal::new();
        journal.noted("scan", 0.5, vec!["2 units".to_string()]);
        journal.run("judge", |_| Done::ok(()));
        let tree = journal.tree();
        assert_eq!(tree.len(), 2);
        assert_eq!(tree[0].op, "scan");
        assert_eq!(tree[0].seconds, 0.5);
        assert_eq!(tree[0].evidence, ["2 units"]);
    }
}
