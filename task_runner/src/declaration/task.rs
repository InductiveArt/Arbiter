use std::any::type_name;
use std::any::Any;
use std::any::TypeId;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;

use crate::context::task_context::TaskContext;
use crate::declaration::pull::Pull;
use crate::declaration::read::Read;
use crate::declaration::read::ReadInput;
use crate::record::task_failure::TaskFailure;
use crate::value::value_type::ValueType;

// The body as the process running it holds it: inputs in, encoded output out.
pub(crate) type TaskBody =
    Arc<dyn Fn(&mut TaskContext, Vec<ReadInput>) -> Result<Vec<u8>, TaskFailure> + Send + Sync>;

// A named unit of work: what it leaves said in one line, the reads it
// declares, the tasks it runs after, the one type it produces, or nothing,
// the body turning the first into the
// second, how long one attempt may take, how many more attempts a failure
// earns, and whether an output once computed for the same inputs is taken
// from the store instead of computed again. A task is found by name in the
// process that runs it, so the name is its identity.
#[derive(Clone)]
pub struct Task {
    pub name: String,
    pub line: String,
    pub reads: Vec<Read>,
    pub produces: ValueType,
    body: TaskBody,
    pub timeout: Option<Duration>,
    pub retries: u32,
    pub cached: bool,
}

impl Task {
    pub fn new<I, O, F>(name: &str, body: F) -> Task
    where
        I: Pull,
        O: Serialize + Any,
        F: Fn(&mut TaskContext, I) -> Result<O, TaskFailure> + Send + Sync + 'static,
    {
        Task {
            name: name.to_string(),
            line: String::new(),
            reads: I::reads(),
            produces: ValueType::of::<O>(),
            body: Arc::new(move |context, inputs| {
                let out = body(context, I::gather(inputs)?)?;
                serde_json::to_vec(&out).map_err(|error| {
                    TaskFailure::from(format!(
                        "the output of type {} does not encode: {error}",
                        type_name::<O>()
                    ))
                })
            }),
            timeout: None,
            retries: 0,
            cached: false,
        }
    }

    pub fn line(mut self, line: &str) -> Task {
        self.line = line.to_string();
        self
    }

    // Nothing is no type: any number of tasks may leave it, and none reads it.
    pub fn leaves_nothing(&self) -> bool {
        self.produces.id == TypeId::of::<()>()
    }

    pub fn within(mut self, timeout: Duration) -> Task {
        self.timeout = Some(timeout);
        self
    }

    pub fn retries(mut self, more: u32) -> Task {
        self.retries = more;
        self
    }

    pub fn cached(mut self) -> Task {
        self.cached = true;
        self
    }

    pub(crate) fn perform(
        &self,
        context: &mut TaskContext,
        inputs: Vec<ReadInput>,
    ) -> Result<Vec<u8>, TaskFailure> {
        (self.body)(context, inputs)
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::time::Duration;

    use serde::Deserialize;
    use serde::Serialize;

    use super::Task;
    use crate::context::stop_token::StopToken;
    use crate::context::task_context::TaskContext;
    use crate::contract::RunId;
    use crate::contract::RunRecord;
    use crate::contract::TaskAttemptId;
    use crate::declaration::pull::Done;
    use crate::declaration::pull::Failed;
    use crate::declaration::pull::In;
    use crate::declaration::read::Read;
    use crate::declaration::read::ReadInput;
    use crate::declaration::read::When;
    use crate::record::task_failure::TaskFailure;
    use crate::value::encoded_value::EncodedValue;
    use crate::value::value_type::ValueType;

    #[derive(Serialize, Deserialize)]
    struct Repo;
    #[derive(Serialize, Deserialize)]
    struct Graph(usize);

    fn attempt_id(path: &str) -> TaskAttemptId {
        TaskAttemptId::new(RunId::next(), path, 1)
    }

    fn context(path: &str) -> TaskContext {
        TaskContext::new(attempt_id(path), StopToken::new(), env::temp_dir())
    }

    fn value<T: Serialize>(value: &T) -> ReadInput {
        ReadInput {
            value: Some(EncodedValue::encoded(value).expect("encodes")),
            completion: None,
        }
    }

    #[test]
    fn a_task_declares_its_reads_from_its_signature() {
        let task = Task::new(
            "resolve",
            |_: &mut TaskContext, (_repo, _graph): (In<Repo>, In<Graph>)| Ok::<u8, TaskFailure>(1),
        );
        assert_eq!(
            task.reads,
            [
                Read::of::<Repo>(When::Passed),
                Read::of::<Graph>(When::Passed)
            ]
        );
        assert_eq!(task.produces, ValueType::of::<u8>());
    }

    #[test]
    fn a_task_declares_when_each_read_fires() {
        let task = Task::new(
            "revert",
            |_: &mut TaskContext, (_bad, _graph, _done): (Failed<Repo>, In<Graph>, Done<Repo>)| {
                Ok::<u8, TaskFailure>(1)
            },
        );
        let whens: Vec<When> = task.reads.iter().map(|read| read.when).collect();
        assert_eq!(whens, [When::Failed, When::Passed, When::Finished]);
    }

    #[test]
    fn a_decorated_body_keeps_the_task_s_declaration() {
        fn noted<I, O, F>(
            body: F,
        ) -> impl Fn(&mut TaskContext, I) -> Result<O, TaskFailure> + Send + Sync + 'static
        where
            F: Fn(&mut TaskContext, I) -> Result<O, TaskFailure> + Send + Sync + 'static,
        {
            move |context, input| {
                context.note("decorated");
                body(context, input)
            }
        }
        let plain = Task::new(
            "resolve",
            |_: &mut TaskContext, (_repo, _graph): (In<Repo>, In<Graph>)| Ok::<u8, TaskFailure>(1),
        );
        let decorated = Task::new(
            "resolve",
            noted(
                |_: &mut TaskContext, (_repo, _graph): (In<Repo>, In<Graph>)| {
                    Ok::<u8, TaskFailure>(1)
                },
            ),
        );
        assert_eq!(decorated.reads, plain.reads);
        assert_eq!(decorated.produces, plain.produces);
        let mut context = context("resolve");
        decorated
            .perform(&mut context, vec![value(&Repo), value(&Graph(1))])
            .expect("the body runs under the decoration");
        let attempt = context.finished(RunRecord::passed());
        assert_eq!(attempt.evidence, ["decorated"]);
    }

    #[test]
    fn a_task_reading_nothing_declares_no_reads() {
        let task = Task::new("scan", |_: &mut TaskContext, (): ()| {
            Ok::<u8, TaskFailure>(0)
        });
        assert!(task.reads.is_empty());
    }

    #[test]
    fn a_task_allows_one_attempt_of_any_length_and_computes_every_time_unless_told_otherwise() {
        let plain = Task::new("scan", |_: &mut TaskContext, (): ()| {
            Ok::<u8, TaskFailure>(0)
        });
        assert_eq!(
            (plain.timeout, plain.retries, plain.cached),
            (None, 0, false)
        );
        let bounded = Task::new("scan", |_: &mut TaskContext, (): ()| {
            Ok::<u8, TaskFailure>(0)
        })
        .within(Duration::from_secs(30))
        .retries(2)
        .cached();
        assert_eq!(bounded.timeout, Some(Duration::from_secs(30)));
        assert_eq!((bounded.retries, bounded.cached), (2, true));
    }

    #[test]
    fn a_task_says_in_one_line_what_it_leaves_once_told() {
        let plain = Task::new("scan", |_: &mut TaskContext, (): ()| {
            Ok::<u8, TaskFailure>(0)
        });
        assert_eq!(plain.line, "");
        let told = plain.line("every file of the unit");
        assert_eq!(told.line, "every file of the unit");
    }

    #[test]
    fn a_task_performs_its_body_on_the_decoded_inputs_and_encodes_its_output() {
        let task = Task::new("sum", |_: &mut TaskContext, (a, b): (In<u8>, In<u8>)| {
            Ok::<u8, TaskFailure>(*a + *b)
        });
        let mut context = context("sum");
        let out = task
            .perform(&mut context, vec![value(&3u8), value(&4u8)])
            .expect("the body adds");
        assert_eq!(out, b"7");
    }

    #[test]
    fn a_value_that_does_not_decode_as_its_type_fails_the_attempt_by_name() {
        let task = Task::new("sum", |_: &mut TaskContext, a: In<u8>| {
            Ok::<u8, TaskFailure>(*a)
        });
        let mut context = context("sum");
        let failed = task
            .perform(&mut context, vec![value(&"seven")])
            .expect_err("a string is not a u8");
        assert!(
            failed
                .reason
                .starts_with("the value handed as u8 does not decode"),
            "{}",
            failed.reason
        );
    }
}
