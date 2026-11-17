use std::process::ExitCode;

use task_runner::declaration::caching::Caching;
use task_runner::declaration::read::Read;
use task_runner::declaration::read::When;
use task_runner::declaration::task::Task;

use crate::gate::listed;

pub fn run() -> ExitCode {
    print!("{}", shown(&listed()));
    ExitCode::SUCCESS
}

// Each task as it declares itself, with no plan and no run: what it leaves
// in one line, each type it reads and when that read fires, the type it
// leaves, and what bounds an attempt.
pub fn shown(tasks: &[Task]) -> String {
    let blocks: Vec<String> = tasks
        .iter()
        .map(|task| {
            let mut lines = vec![task.name.clone()];
            if !task.line.is_empty() {
                lines.push(format!("  {}", task.line));
            }
            let reads: Vec<String> = task.reads.iter().map(fired).collect();
            let reads = if reads.is_empty() {
                "nothing".to_string()
            } else {
                reads.join(", ")
            };
            lines.push(format!("  reads   {reads}"));
            let leaves = if task.leaves_nothing() {
                "nothing".to_string()
            } else {
                short(task.produces.name)
            };
            lines.push(format!("  leaves  {leaves}"));
            lines.push(format!("  limits  {}", limits(task)));
            lines.join("\n") + "\n"
        })
        .collect();
    blocks.join("\n")
}

fn fired(read: &Read) -> String {
    let name = short(read.value_type.name);
    match read.when {
        When::Passed => name,
        When::Failed => format!("{name} when its task failed"),
        When::Finished => format!("{name} however its task ended"),
    }
}

fn limits(task: &Task) -> String {
    let time = match task.timeout {
        None => "no time limit".to_string(),
        Some(limit) => format!("{limit:?} at most"),
    };
    let attempts = match task.retries {
        0 => "one attempt".to_string(),
        more => format!("up to {} attempts", more + 1),
    };
    let kept = match task.caching {
        Caching::Never => "computed every run",
        Caching::Every => "replayed for the same inputs",
        Caching::Accepted(_) => {
            "replayed for the same inputs once its cache test accepted the output"
        }
    };
    format!("{time}, {attempts}, {kept}")
}

// A type's own name, each path of modules leading to a name dropped.
fn short(name: &str) -> String {
    let mut out = String::new();
    let mut path = String::new();
    for letter in name.chars() {
        if letter.is_alphanumeric() || letter == '_' || letter == ':' {
            path.push(letter);
        } else {
            out.push_str(path.rsplit("::").next().unwrap_or_default());
            path.clear();
            out.push(letter);
        }
    }
    out.push_str(path.rsplit("::").next().unwrap_or_default());
    out
}
