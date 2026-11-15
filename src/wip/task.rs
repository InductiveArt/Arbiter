use std::any::type_name;
use std::any::Any;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;

use crate::wip::pull::Pull;
use crate::wip::scope::Scope;
use crate::wip::store::Need;
use crate::wip::store::Part;
use crate::wip::store::Read;
use crate::wip::trace::outcome::Failure;

// The body as the process running it holds it: parts in, encoded product out.
pub type Body = Arc<dyn Fn(&mut Scope, Vec<Part>) -> Result<Vec<u8>, Failure> + Send + Sync>;

// A named unit of work: what it leaves said in one line, the reads it
// declares, the one type it produces, the body turning the first into the
// second, how long one attempt may take, how many more attempts a failure
// earns, and whether a product once computed for the same inputs is taken
// from the store instead of computed again. A task is found by name in the
// process that runs it, so the name is its identity.
#[derive(Clone)]
pub struct Task {
    pub name: String,
    pub line: String,
    pub reads: Vec<Read>,
    pub produces: Need,
    body: Body,
    pub timeout: Option<Duration>,
    pub retries: u32,
    pub cached: bool,
}

impl Task {
    pub fn new<I, O, F>(name: &str, body: F) -> Task
    where
        I: Pull,
        O: Serialize + Any,
        F: Fn(&mut Scope, I) -> Result<O, Failure> + Send + Sync + 'static,
    {
        Task {
            name: name.to_string(),
            line: String::new(),
            reads: I::reads(),
            produces: Need::of::<O>(),
            body: Arc::new(move |scope, parts| {
                let out = body(scope, I::gather(parts)?)?;
                serde_json::to_vec(&out).map_err(|error| {
                    Failure::from(format!(
                        "the product of type {} does not encode: {error}",
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

    pub fn perform(&self, scope: &mut Scope, parts: Vec<Part>) -> Result<Vec<u8>, Failure> {
        (self.body)(scope, parts)
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::time::Duration;

    use serde::Deserialize;
    use serde::Serialize;

    use super::Task;
    use crate::wip::job::cancel::Cancel;
    use crate::wip::job::id::JobId;
    use crate::wip::job::id::RunId;
    use crate::wip::layout::Site;
    use crate::wip::pull::Done;
    use crate::wip::pull::Failed;
    use crate::wip::pull::In;
    use crate::wip::scope::Scope;
    use crate::wip::store::Need;
    use crate::wip::store::Part;
    use crate::wip::store::Product;
    use crate::wip::store::Read;
    use crate::wip::store::When;
    use crate::wip::trace::outcome::Failure;
    use crate::wip::trace::outcome::Outcome;

    #[derive(Serialize, Deserialize)]
    struct Repo;
    #[derive(Serialize, Deserialize)]
    struct Graph(usize);

    fn job(path: &str) -> JobId {
        JobId::new(RunId::next(), path, 1)
    }

    fn scope(path: &str) -> Scope {
        let site = Site {
            dir: env::temp_dir(),
            store: env::temp_dir(),
        };
        Scope::new(job(path), Cancel::new(), site)
    }

    fn value<T: Serialize>(value: &T) -> Part {
        Part {
            value: Some(Product::encoded(value).expect("encodes")),
            completion: None,
        }
    }

    #[test]
    fn a_task_declares_its_reads_from_its_signature() {
        let task = Task::new(
            "resolve",
            |_: &mut Scope, (_repo, _graph): (In<Repo>, In<Graph>)| Ok::<u8, Failure>(1),
        );
        assert_eq!(
            task.reads,
            [
                Read::of::<Repo>(When::Passed),
                Read::of::<Graph>(When::Passed)
            ]
        );
        assert_eq!(task.produces, Need::of::<u8>());
    }

    #[test]
    fn a_task_declares_when_each_read_fires() {
        let task = Task::new(
            "revert",
            |_: &mut Scope, (_bad, _graph, _done): (Failed<Repo>, In<Graph>, Done<Repo>)| {
                Ok::<u8, Failure>(1)
            },
        );
        let whens: Vec<When> = task.reads.iter().map(|read| read.when).collect();
        assert_eq!(whens, [When::Failed, When::Passed, When::Finished]);
    }

    #[test]
    fn a_decorated_body_keeps_the_task_s_declaration() {
        fn noted<I, O, F>(
            body: F,
        ) -> impl Fn(&mut Scope, I) -> Result<O, Failure> + Send + Sync + 'static
        where
            F: Fn(&mut Scope, I) -> Result<O, Failure> + Send + Sync + 'static,
        {
            move |scope, input| {
                scope.note("decorated");
                body(scope, input)
            }
        }
        let plain = Task::new(
            "resolve",
            |_: &mut Scope, (_repo, _graph): (In<Repo>, In<Graph>)| Ok::<u8, Failure>(1),
        );
        let decorated = Task::new(
            "resolve",
            noted(|_: &mut Scope, (_repo, _graph): (In<Repo>, In<Graph>)| Ok::<u8, Failure>(1)),
        );
        assert_eq!(decorated.reads, plain.reads);
        assert_eq!(decorated.produces, plain.produces);
        let mut scope = scope("resolve");
        decorated
            .perform(&mut scope, vec![value(&Repo), value(&Graph(1))])
            .expect("the body runs under the decoration");
        let step = scope.finished("resolve", Outcome::Ok);
        assert_eq!(step.evidence, ["decorated"]);
    }

    #[test]
    fn a_task_reading_nothing_declares_no_reads() {
        let task = Task::new("scan", |_: &mut Scope, (): ()| Ok::<u8, Failure>(0));
        assert!(task.reads.is_empty());
    }

    #[test]
    fn a_task_allows_one_attempt_of_any_length_and_computes_every_time_unless_told_otherwise() {
        let plain = Task::new("scan", |_: &mut Scope, (): ()| Ok::<u8, Failure>(0));
        assert_eq!(
            (plain.timeout, plain.retries, plain.cached),
            (None, 0, false)
        );
        let bounded = Task::new("scan", |_: &mut Scope, (): ()| Ok::<u8, Failure>(0))
            .within(Duration::from_secs(30))
            .retries(2)
            .cached();
        assert_eq!(bounded.timeout, Some(Duration::from_secs(30)));
        assert_eq!((bounded.retries, bounded.cached), (2, true));
    }

    #[test]
    fn a_task_says_in_one_line_what_it_leaves_once_told() {
        let plain = Task::new("scan", |_: &mut Scope, (): ()| Ok::<u8, Failure>(0));
        assert_eq!(plain.line, "");
        let told = plain.line("every file of the unit");
        assert_eq!(told.line, "every file of the unit");
    }

    #[test]
    fn a_task_performs_its_body_on_the_decoded_inputs_and_encodes_its_product() {
        let task = Task::new("sum", |_: &mut Scope, (a, b): (In<u8>, In<u8>)| {
            Ok::<u8, Failure>(*a + *b)
        });
        let mut scope = scope("sum");
        let out = task
            .perform(&mut scope, vec![value(&3u8), value(&4u8)])
            .expect("the body adds");
        assert_eq!(out, b"7");
    }

    #[test]
    fn a_value_that_does_not_decode_as_its_type_fails_the_attempt_by_name() {
        let task = Task::new("sum", |_: &mut Scope, a: In<u8>| Ok::<u8, Failure>(*a));
        let mut scope = scope("sum");
        let failed = task
            .perform(&mut scope, vec![value(&"seven")])
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
