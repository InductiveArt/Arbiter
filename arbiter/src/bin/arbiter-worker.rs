use std::env;
use std::process::ExitCode;

use arbiter::wip_tasks::listed;
use task_runner::worker::serve;

// The worker the runner starts for arbiter's tasks: one process per attempt,
// handed a task's name and finding its body among these, so a task not listed
// cannot run. Started as `arbiter-worker worker <attempt folder>`.
fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    match arguments.first().map(String::as_str) {
        Some("worker") => serve(&arguments[1..], &listed()),
        _ => {
            eprintln!("arbiter-worker: run as `arbiter-worker worker <attempt folder>`");
            ExitCode::from(2)
        }
    }
}
