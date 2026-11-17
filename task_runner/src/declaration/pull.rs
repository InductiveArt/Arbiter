use std::any::type_name;
use std::any::Any;
use std::marker::PhantomData;
use std::ops::Deref;

use serde::de::DeserializeOwned;

use crate::contract::run_record::Ended;
use crate::contract::RunRecord;
use crate::contract::TaskAttemptId;
use crate::declaration::read::Read;
use crate::declaration::read::ReadInput;
use crate::declaration::read::When;
use crate::record::completion::Completion;
use crate::record::task_failure::TaskFailure;

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
// spent. Its attempt id names the attempt that ended it.
pub struct Failed<T> {
    pub attempt_id: TaskAttemptId,
    pub reason: String,
    of: PhantomData<T>,
}

// How the task producing T finished, read by a task that runs either way: the
// outcome, and the value when it left one.
pub struct Done<T> {
    pub attempt_id: TaskAttemptId,
    pub outcome: RunRecord,
    value: Option<T>,
}

impl<T> Done<T> {
    pub fn value(&self) -> Option<&T> {
        self.value.as_ref()
    }
}

// One input, declared as a read and built back from what was handed for it.
// An input that does not decode as the type is a failure of the attempt, named
// by the type it was meant to be.
pub trait Pulled: Sized {
    fn read() -> Read;
    fn pulled(input: ReadInput) -> Result<Self, TaskFailure>;
}

fn decoded<T: DeserializeOwned>(input: &ReadInput) -> Result<Option<T>, TaskFailure> {
    match &input.value {
        None => Ok(None),
        Some(value) => value.decoded().map(Some).map_err(|error| {
            TaskFailure::from(format!(
                "the value handed as {} does not decode: {error}",
                type_name::<T>()
            ))
        }),
    }
}

fn completion(input: &ReadInput) -> Result<Completion, TaskFailure> {
    input.completion.clone().ok_or_else(|| {
        TaskFailure::from("a read fired by failing or finishing was handed no completion")
    })
}

impl<T: DeserializeOwned + Any> Pulled for In<T> {
    fn read() -> Read {
        Read::of::<T>(When::Passed)
    }

    fn pulled(input: ReadInput) -> Result<Self, TaskFailure> {
        let value = decoded::<T>(&input)?.ok_or_else(|| {
            TaskFailure::from(format!("no value was handed for {}", type_name::<T>()))
        })?;
        Ok(In { value })
    }
}

impl<T: DeserializeOwned + Any> Pulled for Failed<T> {
    fn read() -> Read {
        Read::of::<T>(When::Failed)
    }

    fn pulled(input: ReadInput) -> Result<Self, TaskFailure> {
        let completion = completion(&input)?;
        let reason = match &completion.outcome.ended {
            Some(Ended::Failed(reason)) => reason.clone(),
            _ => completion.outcome.to_string(),
        };
        Ok(Failed {
            attempt_id: completion.attempt_id,
            reason,
            of: PhantomData,
        })
    }
}

impl<T: DeserializeOwned + Any> Pulled for Done<T> {
    fn read() -> Read {
        Read::of::<T>(When::Finished)
    }

    fn pulled(input: ReadInput) -> Result<Self, TaskFailure> {
        let completion = completion(&input)?;
        Ok(Done {
            attempt_id: completion.attempt_id,
            outcome: completion.outcome,
            value: decoded::<T>(&input)?,
        })
    }
}

// Everything a task reads, as the type of its body's parameter: the signature
// is the declaration, so a task cannot read what it did not declare.
pub trait Pull: Sized {
    fn reads() -> Vec<Read>;
    fn gather(inputs: Vec<ReadInput>) -> Result<Self, TaskFailure>;
}

impl Pull for () {
    fn reads() -> Vec<Read> {
        Vec::new()
    }

    fn gather(_: Vec<ReadInput>) -> Result<Self, TaskFailure> {
        Ok(())
    }
}

impl<P: Pulled> Pull for P {
    fn reads() -> Vec<Read> {
        vec![P::read()]
    }

    fn gather(inputs: Vec<ReadInput>) -> Result<Self, TaskFailure> {
        let mut inputs = inputs.into_iter();
        P::pulled(inputs.next().ok_or("one input per read")?)
    }
}

macro_rules! pull_tuple {
    ($($input:ident),+) => {
        impl<$($input: Pulled),+> Pull for ($($input,)+) {
            fn reads() -> Vec<Read> {
                vec![$($input::read()),+]
            }

            fn gather(inputs: Vec<ReadInput>) -> Result<Self, TaskFailure> {
                let mut inputs = inputs.into_iter();
                Ok(($($input::pulled(inputs.next().ok_or("one input per read")?)?,)+))
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
    use crate::contract::RunId;
    use crate::contract::RunRecord;
    use crate::contract::TaskAttemptId;
    use crate::declaration::read::ReadInput;
    use crate::record::completion::Completion;
    use crate::value::encoded_value::EncodedValue;

    #[derive(Serialize, Deserialize)]
    struct Repo;
    #[derive(Serialize, Deserialize)]
    struct Graph(usize);

    fn attempt_id(path: &str) -> TaskAttemptId {
        TaskAttemptId::new(RunId::next(), path, 1)
    }

    #[test]
    fn a_failure_read_carries_who_failed_and_why() {
        let input = ReadInput {
            value: None,
            completion: Some(Completion {
                attempt_id: attempt_id("gate/check"),
                outcome: RunRecord::failed("does not compile".to_string()),
            }),
        };
        let failed = <Failed<Repo> as Pulled>::pulled(input).expect("pulled");
        assert_eq!(failed.attempt_id.path(), "gate/check");
        assert_eq!(failed.reason, "does not compile");
        let late = ReadInput {
            value: None,
            completion: Some(Completion {
                attempt_id: TaskAttemptId::new(RunId::next(), "gate/check", 3),
                outcome: RunRecord::timed_out(),
            }),
        };
        let failed = <Failed<Repo> as Pulled>::pulled(late).expect("pulled");
        assert_eq!(
            (failed.attempt_id.attempt, failed.reason.as_str()),
            (3, "timed out")
        );
    }

    #[test]
    fn a_finishing_read_carries_the_outcome_and_the_value_left() {
        let passed = ReadInput {
            value: Some(EncodedValue::encoded(&Graph(41)).expect("encodes")),
            completion: Some(Completion {
                attempt_id: attempt_id("gate/resolve"),
                outcome: RunRecord::passed(),
            }),
        };
        let done = <Done<Graph> as Pulled>::pulled(passed).expect("pulled");
        assert_eq!(done.attempt_id.path(), "gate/resolve");
        assert_eq!(done.outcome, RunRecord::passed());
        assert_eq!(done.value().map(|graph| graph.0), Some(41));
        let skipped = ReadInput {
            value: None,
            completion: Some(Completion {
                attempt_id: TaskAttemptId::new(RunId::next(), "gate/resolve", 0),
                outcome: RunRecord::skipped("gate/scan".to_string()),
            }),
        };
        let done = <Done<Graph> as Pulled>::pulled(skipped).expect("pulled");
        assert_eq!(done.outcome, RunRecord::skipped("gate/scan".to_string()));
        assert!(done.value().is_none());
    }
}
