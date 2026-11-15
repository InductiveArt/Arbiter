use std::env;
use std::process::ExitCode;

use task_runner::registry::known;
use task_runner::worker::serve;

// The worker this unit's own tests start: one process per attempt at one of
// the exhibit's tasks, started as `exhibit-worker worker <attempt folder>`.
fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    match arguments.first().map(String::as_str) {
        Some("worker") => serve(&arguments[1..], &known()),
        _ => {
            eprintln!("exhibit-worker: run as `exhibit-worker worker <attempt folder>`");
            ExitCode::from(2)
        }
    }
}
