use std::env;
use std::process::ExitCode;

use arbiter::wip_tasks::listed;
use task_runner::attempt::task_process::run_task_process;

// Arbiter's tasks as one executable: the runner starts it once per attempt as
// `arbiter-tasks <attempt folder>`, and it performs the task the order in that
// folder names, finding its body among these, so a task not listed cannot run.
fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    run_task_process(&arguments, &listed())
}
