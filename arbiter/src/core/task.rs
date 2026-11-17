use std::process::ExitCode;

// A named unit of work: it parses its own arguments, does the work and reports.
pub struct Task {
    pub name: &'static str,
    pub line: &'static str,
    pub run: fn(&[String]) -> ExitCode,
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
