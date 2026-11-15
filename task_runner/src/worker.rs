use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitCode;

use serde::Deserialize;
use serde::Serialize;

use crate::job::cancel::Cancel;
use crate::job::id::JobId;
use crate::kernel::signal::stop_on_terminate;
use crate::layout::Site;
use crate::layout::ORDER_FILE;
use crate::layout::PRODUCT_FILE;
use crate::layout::RECORD_FILE;
use crate::layout::STARTED_FILE;
use crate::scope::Scope;
use crate::store::Completion;
use crate::store::Part;
use crate::store::Product;
use crate::store::When;
use crate::task::Task;
use crate::trace::outcome::Failure;
use crate::trace::outcome::Outcome;

// What the run writes for the process performing an attempt: which attempt,
// which task by name, where the run's store is, the identity of the code the
// run keys its products by, and each read in the task's order, as a file to
// decode, a completion, or both.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Order {
    pub id: JobId,
    pub task: String,
    pub store: PathBuf,
    pub identity: u64,
    pub inputs: Vec<Input>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Input {
    pub need: String,
    pub when: When,
    pub value: Option<PathBuf>,
    pub completion: Option<Completion>,
}

// Performs one attempt as a process of its own: the task the order names, on
// the inputs it names, leaving the product and the record in the attempt's
// folder. Exits 0 when the attempt passed, 1 when it did not, and 2 when no
// attempt could be made or recorded at all.
pub fn serve(args: &[String], tasks: &[Task]) -> ExitCode {
    ExitCode::from(perform(args, tasks))
}

pub fn perform(args: &[String], tasks: &[Task]) -> u8 {
    let Some(dir) = args.first().map(PathBuf::from) else {
        eprintln!("worker: no attempt folder was given");
        return 2;
    };
    let order = match ordered(&dir) {
        Ok(order) => order,
        Err(error) => {
            eprintln!("worker: {error}");
            return 2;
        }
    };
    stop_on_terminate();
    let _ = fs::write(dir.join(STARTED_FILE), b"");
    let site = Site {
        dir: dir.clone(),
        store: order.store.clone(),
    };
    let mut scope = Scope::new(order.id.clone(), Cancel::new(), site).identified(order.identity);
    let performed = match tasks.iter().find(|task| task.name == order.task) {
        None => Err(Failure::from(format!(
            "this binary knows no task named {}",
            order.task
        ))),
        Some(task) => handed(task, &order).and_then(|parts| task.perform(&mut scope, parts)),
    };
    let outcome = match performed {
        Ok(bytes) => match fs::write(dir.join(PRODUCT_FILE), bytes) {
            Ok(()) => Outcome::Ok,
            Err(error) => Outcome::Failed(format!("the product could not be written: {error}")),
        },
        Err(failure) => Outcome::Failed(failure.reason),
    };
    let step = scope.finished(&order.task, outcome.clone());
    let recorded = serde_json::to_vec_pretty(&step)
        .map_err(io::Error::other)
        .and_then(|bytes| fs::write(dir.join(RECORD_FILE), bytes));
    if let Err(error) = recorded {
        eprintln!("worker: the record could not be written: {error}");
        return 2;
    }
    u8::from(outcome != Outcome::Ok)
}

fn ordered(dir: &Path) -> io::Result<Order> {
    let bytes = fs::read(dir.join(ORDER_FILE))?;
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}

// The parts the order hands, checked against what the task declares: the
// same reads in the same order, or the attempt fails before the body runs.
fn handed(task: &Task, order: &Order) -> Result<Vec<Part>, Failure> {
    if order.inputs.len() != task.reads.len() {
        return Err(Failure::from(format!(
            "the order hands {} inputs but the task declares {} reads",
            order.inputs.len(),
            task.reads.len()
        )));
    }
    let mut parts = Vec::with_capacity(task.reads.len());
    for (n, (input, read)) in order.inputs.iter().zip(&task.reads).enumerate() {
        if input.need != read.need.name || input.when != read.when {
            return Err(Failure::from(format!(
                "input {n} is {} fired by {:?}, but the task reads {} fired by {:?}",
                input.need, input.when, read.need.name, read.when
            )));
        }
        let value = match &input.value {
            None => None,
            Some(file) => Some(Product::from_file(file.clone()).map_err(|error| {
                Failure::from(format!(
                    "the value for {} could not be read from {}: {error}",
                    read.need.name,
                    file.display()
                ))
            })?),
        };
        parts.push(Part {
            value,
            completion: input.completion.clone(),
        });
    }
    Ok(parts)
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;

    use super::perform;
    use super::Input;
    use super::Order;
    use crate::job::id::JobId;
    use crate::job::id::RunId;
    use crate::layout::PRODUCT_FILE;
    use crate::layout::RECORD_FILE;
    use crate::pull::In;
    use crate::scope::Scope;
    use crate::store::When;
    use crate::task::Task;
    use crate::trace::outcome::Failure;
    use crate::trace::outcome::Outcome;
    use crate::trace::step::Step;

    fn double() -> Task {
        Task::new("double", |scope: &mut Scope, n: In<u8>| {
            scope.note("doubled");
            Ok::<u8, Failure>(*n * 2)
        })
    }

    fn folder(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("task-runner-worker-{name}-{}", RunId::next()));
        fs::create_dir_all(&dir).expect("a folder is made");
        dir
    }

    fn ordered(dir: &Path, task: &str, need: &str) -> Order {
        let seed = dir.join("seed.json");
        fs::write(&seed, b"7").expect("the seed is written");
        let order = Order {
            id: JobId::new(RunId::next(), format!("p/{task}"), 1),
            task: task.to_string(),
            store: dir.join("store"),
            identity: 0,
            inputs: vec![Input {
                need: need.to_string(),
                when: When::Passed,
                value: Some(seed),
                completion: None,
            }],
        };
        let bytes = serde_json::to_vec_pretty(&order).expect("an order encodes");
        fs::write(dir.join("job.json"), bytes).expect("the order is written");
        order
    }

    fn record(dir: &Path) -> Step {
        let bytes = fs::read(dir.join(RECORD_FILE)).expect("a record is left");
        serde_json::from_slice(&bytes).expect("a record decodes")
    }

    #[test]
    fn a_worker_performs_the_named_task_on_the_inputs_handed_and_leaves_its_product_and_record() {
        let dir = folder("double");
        let order = ordered(&dir, "double", "u8");
        let code = perform(&[dir.display().to_string()], &[double()]);
        assert_eq!(code, 0);
        assert_eq!(
            fs::read(dir.join(PRODUCT_FILE)).expect("a product is left"),
            b"14"
        );
        let step = record(&dir);
        assert_eq!(step.job, order.id);
        assert_eq!(step.outcome, Outcome::Ok);
        assert_eq!(step.evidence, ["doubled"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_worker_asked_for_a_task_it_does_not_know_leaves_a_failed_record() {
        let dir = folder("unknown");
        ordered(&dir, "triple", "u8");
        let code = perform(&[dir.display().to_string()], &[double()]);
        assert_eq!(code, 1);
        assert_eq!(
            record(&dir).outcome,
            Outcome::Failed("this binary knows no task named triple".to_string())
        );
        assert!(!dir.join(PRODUCT_FILE).exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_worker_handed_inputs_the_task_does_not_declare_fails_before_the_body_runs() {
        let dir = folder("mismatch");
        ordered(&dir, "double", "u16");
        let code = perform(&[dir.display().to_string()], &[double()]);
        assert_eq!(code, 1);
        let step = record(&dir);
        assert!(
            matches!(&step.outcome, Outcome::Failed(reason) if reason.starts_with("input 0 is u16")),
            "{:?}",
            step.outcome
        );
        assert!(step.evidence.is_empty(), "the body never ran");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_worker_without_an_order_makes_no_attempt() {
        assert_eq!(perform(&[], &[double()]), 2);
        let dir = folder("empty");
        assert_eq!(perform(&[dir.display().to_string()], &[double()]), 2);
        assert!(!dir.join(RECORD_FILE).exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
