use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitCode;

use crate::context::stop_token::StopToken;
use crate::context::task_context::TaskContext;
use crate::contract::RunRecord;
use crate::declaration::read::ReadInput;
use crate::declaration::task::Task;
use crate::kernel::signal::stop_on_terminate;
use crate::protocol::attempt_folder::ORDER_FILE;
use crate::protocol::attempt_folder::OUTPUT_FILE;
use crate::protocol::attempt_folder::RECORD_FILE;
use crate::protocol::attempt_folder::STARTED_FILE;
use crate::protocol::task_order::TaskOrder;
use crate::record::task_failure::TaskFailure;
use crate::value::encoded_value::EncodedValue;

// Performs one attempt as a process of its own: the task the order names, on
// the inputs it names, leaving the output and the record in the attempt's
// folder. Exits 0 when the attempt passed, 1 when it did not, and 2 when no
// attempt could be made or recorded at all.
pub fn run_task_process(args: &[String], tasks: &[Task]) -> ExitCode {
    ExitCode::from(perform_attempt(args, tasks))
}

fn perform_attempt(args: &[String], tasks: &[Task]) -> u8 {
    let Some(dir) = args.first().map(PathBuf::from) else {
        eprintln!("task process: no attempt folder was given");
        return 2;
    };
    let order = match ordered(&dir) {
        Ok(order) => order,
        Err(error) => {
            eprintln!("task process: {error}");
            return 2;
        }
    };
    stop_on_terminate();
    let _ = fs::write(dir.join(STARTED_FILE), b"");
    let mut context = TaskContext::new(order.attempt_id.clone(), StopToken::new(), dir.clone());
    let performed = match tasks.iter().find(|task| task.name == order.task) {
        None => Err(TaskFailure::from(format!(
            "this binary knows no task named {}",
            order.task
        ))),
        Some(task) => handed(task, &order)
            .and_then(|inputs| task.perform(&mut context, inputs))
            .map(|bytes| (task, bytes)),
    };
    let outcome = match performed {
        // A task leaving nothing has no output to write.
        Ok((task, _)) if task.leaves_nothing() => RunRecord::passed(),
        Ok((_, bytes)) => match fs::write(dir.join(OUTPUT_FILE), bytes) {
            Ok(()) => RunRecord::passed(),
            Err(error) => RunRecord::failed(format!("the output could not be written: {error}")),
        },
        Err(failure) => RunRecord::failed(failure.reason),
    };
    let attempt = context.finished(outcome.clone());
    let recorded = attempt.write(&dir.join(RECORD_FILE));
    if let Err(error) = recorded {
        eprintln!("task process: the record could not be written: {error}");
        return 2;
    }
    u8::from(outcome != RunRecord::passed())
}

fn ordered(dir: &Path) -> io::Result<TaskOrder> {
    let bytes = fs::read(dir.join(ORDER_FILE))?;
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}

// The inputs the order hands, checked against what the task declares: the
// same reads in the same order, or the attempt fails before the body runs.
fn handed(task: &Task, order: &TaskOrder) -> Result<Vec<ReadInput>, TaskFailure> {
    if order.inputs.len() != task.reads.len() {
        return Err(TaskFailure::from(format!(
            "the order hands {} inputs but the task declares {} reads",
            order.inputs.len(),
            task.reads.len()
        )));
    }
    let mut inputs = Vec::with_capacity(task.reads.len());
    for (n, (input, read)) in order.inputs.iter().zip(&task.reads).enumerate() {
        if input.value_type != read.value_type.name || input.when != read.when {
            return Err(TaskFailure::from(format!(
                "input {n} is {} fired by {:?}, but the task reads {} fired by {:?}",
                input.value_type, input.when, read.value_type.name, read.when
            )));
        }
        let value = match &input.value {
            None => None,
            Some(file) => Some(EncodedValue::from_file(file.clone()).map_err(|error| {
                TaskFailure::from(format!(
                    "the value for {} could not be read from {}: {error}",
                    read.value_type.name,
                    file.display()
                ))
            })?),
        };
        inputs.push(ReadInput {
            value,
            completion: input.completion.clone(),
        });
    }
    Ok(inputs)
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;

    use super::perform_attempt;
    use crate::context::task_context::TaskContext;
    use crate::contract::run_record::Ended;
    use crate::contract::RunId;
    use crate::contract::RunRecord;
    use crate::contract::TaskAttempt;
    use crate::contract::TaskAttemptId;
    use crate::declaration::pull::In;
    use crate::declaration::read::When;
    use crate::declaration::task::Task;
    use crate::protocol::attempt_folder::ORDER_FILE;
    use crate::protocol::attempt_folder::OUTPUT_FILE;
    use crate::protocol::attempt_folder::RECORD_FILE;
    use crate::protocol::task_order::TaskOrder;
    use crate::protocol::task_order_input::TaskOrderInput;
    use crate::record::task_failure::TaskFailure;

    fn double() -> Task {
        Task::new("double", |context: &mut TaskContext, n: In<u8>| {
            context.note("doubled");
            Ok::<u8, TaskFailure>(*n * 2)
        })
    }

    fn folder(name: &str) -> PathBuf {
        let dir =
            env::temp_dir().join(format!("task-runner-task-process-{name}-{}", RunId::next()));
        fs::create_dir_all(&dir).expect("a folder is made");
        dir
    }

    fn ordered(dir: &Path, task: &str, value_type: &str) -> TaskOrder {
        let seed = dir.join("seed.json");
        fs::write(&seed, b"7").expect("the seed is written");
        let order = TaskOrder {
            attempt_id: TaskAttemptId::new(RunId::next(), format!("p/{task}"), 1),
            task: task.to_string(),
            inputs: vec![TaskOrderInput {
                value_type: value_type.to_string(),
                when: When::Passed,
                value: Some(seed),
                completion: None,
            }],
        };
        let bytes = serde_json::to_vec_pretty(&order).expect("an order encodes");
        fs::write(dir.join(ORDER_FILE), bytes).expect("the order is written");
        order
    }

    fn record(dir: &Path) -> TaskAttempt {
        TaskAttempt::read(&dir.join(RECORD_FILE)).expect("a record is left")
    }

    #[test]
    fn a_task_process_performs_the_named_task_on_the_inputs_handed_and_leaves_its_output_and_record(
    ) {
        let dir = folder("double");
        let order = ordered(&dir, "double", "u8");
        let code = perform_attempt(&[dir.display().to_string()], &[double()]);
        assert_eq!(code, 0);
        assert_eq!(
            fs::read(dir.join(OUTPUT_FILE)).expect("an output is left"),
            b"14"
        );
        let attempt = record(&dir);
        assert_eq!(attempt.id(), &order.attempt_id);
        assert_eq!(*attempt.outcome(), RunRecord::passed());
        assert_eq!(attempt.evidence, ["doubled"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_task_leaving_nothing_passes_with_no_output_file() {
        let dir = folder("nothing");
        ordered(&dir, "note", "u8");
        let note = Task::new("note", |context: &mut TaskContext, _: In<u8>| {
            context.note("noted");
            Ok::<(), TaskFailure>(())
        });
        let code = perform_attempt(&[dir.display().to_string()], &[note]);
        assert_eq!(code, 0);
        assert_eq!(*record(&dir).outcome(), RunRecord::passed());
        assert!(!dir.join(OUTPUT_FILE).exists(), "nothing is no output");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_task_process_asked_for_a_task_it_does_not_know_leaves_a_failed_record() {
        let dir = folder("unknown");
        ordered(&dir, "triple", "u8");
        let code = perform_attempt(&[dir.display().to_string()], &[double()]);
        assert_eq!(code, 1);
        assert_eq!(
            *record(&dir).outcome(),
            RunRecord::failed("this binary knows no task named triple".to_string())
        );
        assert!(!dir.join(OUTPUT_FILE).exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_task_process_handed_inputs_the_task_does_not_declare_fails_before_the_body_runs() {
        let dir = folder("mismatch");
        ordered(&dir, "double", "u16");
        let code = perform_attempt(&[dir.display().to_string()], &[double()]);
        assert_eq!(code, 1);
        let attempt = record(&dir);
        assert!(
            matches!(&attempt.outcome().ended, Some(Ended::Failed(reason)) if reason.starts_with("input 0 is u16")),
            "{:?}",
            attempt.outcome
        );
        assert!(attempt.evidence.is_empty(), "the body never ran");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_task_process_without_an_order_makes_no_attempt() {
        assert_eq!(perform_attempt(&[], &[double()]), 2);
        let dir = folder("empty");
        assert_eq!(
            perform_attempt(&[dir.display().to_string()], &[double()]),
            2
        );
        assert!(!dir.join(RECORD_FILE).exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
