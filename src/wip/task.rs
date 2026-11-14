use std::any::type_name;
use std::any::Any;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::wip::job::JobId;
use crate::wip::store::Completion;
use crate::wip::store::Need;
use crate::wip::store::Part;
use crate::wip::store::Read;
use crate::wip::store::When;
use crate::wip::trace::Failure;
use crate::wip::trace::Outcome;
use crate::wip::trace::Scope;

// One value a task reads, decoded from the bytes its producer left. The read
// fires when the value's producer passed.
pub struct In<T> {
    value: T,
}

impl<T> Deref for In<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.value
    }
}

// The failure of the task producing T, read by a task that runs because of it:
// the read fires only when that producer ended without passing, its retries
// spent. The job names the attempt that ended it.
pub struct Failed<T> {
    pub job: JobId,
    pub reason: String,
    of: PhantomData<T>,
}

// How the task producing T finished, read by a task that runs either way: the
// outcome, and the value when it left one.
pub struct Done<T> {
    pub job: JobId,
    pub outcome: Outcome,
    value: Option<T>,
}

impl<T> Done<T> {
    pub fn value(&self) -> Option<&T> {
        self.value.as_ref()
    }
}

// One input, declared as a read and built back from the part handed for it.
// A part that does not decode as the type is a failure of the attempt, named
// by the type it was meant to be.
pub trait Pulled: Sized {
    fn read() -> Read;
    fn pulled(part: Part) -> Result<Self, Failure>;
}

fn decoded<T: DeserializeOwned>(part: &Part) -> Result<Option<T>, Failure> {
    match &part.value {
        None => Ok(None),
        Some(product) => product.decoded().map(Some).map_err(|error| {
            Failure::from(format!(
                "the value handed as {} does not decode: {error}",
                type_name::<T>()
            ))
        }),
    }
}

fn completion(part: &Part) -> Result<Completion, Failure> {
    part.completion.clone().ok_or_else(|| {
        Failure::from("a read fired by failing or finishing was handed no completion")
    })
}

impl<T: DeserializeOwned + Any> Pulled for In<T> {
    fn read() -> Read {
        Read::of::<T>(When::Passed)
    }

    fn pulled(part: Part) -> Result<Self, Failure> {
        let value = decoded::<T>(&part)?.ok_or_else(|| {
            Failure::from(format!("no value was handed for {}", type_name::<T>()))
        })?;
        Ok(In { value })
    }
}

impl<T: DeserializeOwned + Any> Pulled for Failed<T> {
    fn read() -> Read {
        Read::of::<T>(When::Failed)
    }

    fn pulled(part: Part) -> Result<Self, Failure> {
        let completion = completion(&part)?;
        let reason = match &completion.outcome {
            Outcome::Failed(reason) => reason.clone(),
            Outcome::TimedOut => "timed out".to_string(),
            Outcome::Cancelled => "cancelled".to_string(),
            other => format!("{other:?}"),
        };
        Ok(Failed {
            job: completion.job,
            reason,
            of: PhantomData,
        })
    }
}

impl<T: DeserializeOwned + Any> Pulled for Done<T> {
    fn read() -> Read {
        Read::of::<T>(When::Finished)
    }

    fn pulled(part: Part) -> Result<Self, Failure> {
        let completion = completion(&part)?;
        Ok(Done {
            job: completion.job,
            outcome: completion.outcome,
            value: decoded::<T>(&part)?,
        })
    }
}

// Everything a task reads, as the type of its body's parameter: the signature
// is the declaration, so a task cannot read what it did not declare.
pub trait Pull: Sized {
    fn reads() -> Vec<Read>;
    fn gather(parts: Vec<Part>) -> Result<Self, Failure>;
}

impl Pull for () {
    fn reads() -> Vec<Read> {
        Vec::new()
    }

    fn gather(_: Vec<Part>) -> Result<Self, Failure> {
        Ok(())
    }
}

impl<P: Pulled> Pull for P {
    fn reads() -> Vec<Read> {
        vec![P::read()]
    }

    fn gather(parts: Vec<Part>) -> Result<Self, Failure> {
        let mut parts = parts.into_iter();
        P::pulled(parts.next().ok_or("one part per read")?)
    }
}

macro_rules! pull_tuple {
    ($($part:ident),+) => {
        impl<$($part: Pulled),+> Pull for ($($part,)+) {
            fn reads() -> Vec<Read> {
                vec![$($part::read()),+]
            }

            fn gather(parts: Vec<Part>) -> Result<Self, Failure> {
                let mut parts = parts.into_iter();
                Ok(($($part::pulled(parts.next().ok_or("one part per read")?)?,)+))
            }
        }
    };
}

pull_tuple!(A);
pull_tuple!(A, B);
pull_tuple!(A, B, C);
pull_tuple!(A, B, C, D);

// The body as the process running it holds it: parts in, encoded product out.
pub type Body = Arc<dyn Fn(&mut Scope, Vec<Part>) -> Result<Vec<u8>, Failure> + Send + Sync>;

// A named unit of work: the reads it declares, the one type it produces, the
// body turning the first into the second, how long one attempt may take, how
// many more attempts a failure earns, and whether a product once computed for
// the same inputs is taken from the store instead of computed again. A task
// is found by name in the process that runs it, so the name is its identity.
#[derive(Clone)]
pub struct Task {
    pub name: String,
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

    use super::Done;
    use super::Failed;
    use super::In;
    use super::Pulled;
    use super::Task;
    use crate::wip::job::Cancel;
    use crate::wip::job::JobId;
    use crate::wip::job::RunId;
    use crate::wip::store::Completion;
    use crate::wip::store::Need;
    use crate::wip::store::Part;
    use crate::wip::store::Product;
    use crate::wip::store::Read;
    use crate::wip::store::When;
    use crate::wip::trace::Failure;
    use crate::wip::trace::Outcome;
    use crate::wip::trace::Scope;
    use crate::wip::trace::Site;

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

    #[test]
    fn a_failure_read_carries_who_failed_and_why() {
        let part = Part {
            value: None,
            completion: Some(Completion {
                job: job("gate/check"),
                outcome: Outcome::Failed("does not compile".to_string()),
            }),
        };
        let failed = <Failed<Repo> as Pulled>::pulled(part).expect("pulled");
        assert_eq!(failed.job.path, "gate/check");
        assert_eq!(failed.reason, "does not compile");
        let late = Part {
            value: None,
            completion: Some(Completion {
                job: JobId::new(RunId::next(), "gate/check", 3),
                outcome: Outcome::TimedOut,
            }),
        };
        let failed = <Failed<Repo> as Pulled>::pulled(late).expect("pulled");
        assert_eq!(
            (failed.job.attempt, failed.reason.as_str()),
            (3, "timed out")
        );
    }

    #[test]
    fn a_finishing_read_carries_the_outcome_and_the_value_left() {
        let passed = Part {
            value: Some(Product::encoded(&Graph(41)).expect("encodes")),
            completion: Some(Completion {
                job: job("gate/resolve"),
                outcome: Outcome::Ok,
            }),
        };
        let done = <Done<Graph> as Pulled>::pulled(passed).expect("pulled");
        assert_eq!(done.job.path, "gate/resolve");
        assert_eq!(done.outcome, Outcome::Ok);
        assert_eq!(done.value().map(|graph| graph.0), Some(41));
        let skipped = Part {
            value: None,
            completion: Some(Completion {
                job: JobId::new(RunId::next(), "gate/resolve", 0),
                outcome: Outcome::Skipped("gate/scan".to_string()),
            }),
        };
        let done = <Done<Graph> as Pulled>::pulled(skipped).expect("pulled");
        assert_eq!(done.outcome, Outcome::Skipped("gate/scan".to_string()));
        assert!(done.value().is_none());
    }
}
