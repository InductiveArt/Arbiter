use std::any::type_name;
use std::any::Any;
use std::marker::PhantomData;
use std::ops::Deref;

use serde::de::DeserializeOwned;

use crate::wip::job::id::JobId;
use crate::wip::store::Completion;
use crate::wip::store::Part;
use crate::wip::store::Read;
use crate::wip::store::When;
use crate::wip::trace::outcome::Failure;
use crate::wip::trace::outcome::Outcome;

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

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use serde::Serialize;

    use super::Done;
    use super::Failed;
    use super::Pulled;
    use crate::wip::job::id::JobId;
    use crate::wip::job::id::RunId;
    use crate::wip::store::Completion;
    use crate::wip::store::Part;
    use crate::wip::store::Product;
    use crate::wip::trace::outcome::Outcome;

    #[derive(Serialize, Deserialize)]
    struct Repo;
    #[derive(Serialize, Deserialize)]
    struct Graph(usize);

    fn job(path: &str) -> JobId {
        JobId::new(RunId::next(), path, 1)
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
