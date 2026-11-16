use std::fmt;
use std::path::PathBuf;

use crate::value::value_type::ValueType;

// What stands between a plan and a run, found before any process starts.
#[derive(Debug, PartialEq)]
pub enum RefusalReason {
    // A type some task reads that no task produces and the seeds do not hold.
    Unseeded { value_type: ValueType },
    // A type both seeded and produced: the output would replace the seed.
    Shadowed { value_type: ValueType, by: String },
    // The task executable is missing, not a file, or not executable.
    NoTaskExecutable { path: PathBuf, reason: String },
    // The root is not a folder, or the run's folder or seeds could not be written.
    Unwritable { path: PathBuf, reason: String },
}

impl fmt::Display for RefusalReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RefusalReason::Unseeded { value_type } => {
                write!(
                    f,
                    "{} is read but neither produced nor seeded",
                    value_type.name
                )
            }
            RefusalReason::Shadowed { value_type, by } => {
                write!(f, "{} is seeded and produced by {by}", value_type.name)
            }
            RefusalReason::NoTaskExecutable { path, reason } => {
                write!(
                    f,
                    "the task executable {} cannot run: {reason}",
                    path.display()
                )
            }
            RefusalReason::Unwritable { path, reason } => {
                write!(f, "{} cannot be written: {reason}", path.display())
            }
        }
    }
}

// A run refused whole before any task starts, with every reason found.
#[derive(Debug, PartialEq)]
pub struct Refusal {
    pub reasons: Vec<RefusalReason>,
}

impl From<RefusalReason> for Refusal {
    fn from(reason: RefusalReason) -> Refusal {
        Refusal {
            reasons: vec![reason],
        }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for reason in &self.reasons {
            writeln!(f, "{reason}")?;
        }
        Ok(())
    }
}
