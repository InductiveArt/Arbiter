use std::env;
use std::process::ExitCode;

use task_runner::attempt::task_process::run_task_process;
use task_runner::test_tasks::tasks::listed;

// The runner's test tasks as one executable, for this unit's own tests: the
// runner starts it once per attempt as `test-tasks <attempt folder>`.
fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    run_task_process(&arguments, &listed())
}
