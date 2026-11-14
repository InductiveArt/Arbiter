use std::fmt;
use std::path::PathBuf;

use crate::wip::store::Need;

// What stands between a plan and a run, found before any process starts.
#[derive(Debug, PartialEq)]
pub enum Problem {
    // A type some task reads that no task produces and the seeds do not hold.
    Unseeded { need: Need },
    // A type both seeded and produced: the product would replace the seed.
    Shadowed { need: Need, by: String },
    // The worker binary is missing, not a file, or not executable.
    NoWorker { path: PathBuf, reason: String },
    // The root is not a folder, or the run's folder or seeds could not be written.
    Unwritable { path: PathBuf, reason: String },
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Problem::Unseeded { need } => {
                write!(f, "{} is read but neither produced nor seeded", need.name)
            }
            Problem::Shadowed { need, by } => {
                write!(f, "{} is seeded and produced by {by}", need.name)
            }
            Problem::NoWorker { path, reason } => {
                write!(f, "the worker {} cannot run: {reason}", path.display())
            }
            Problem::Unwritable { path, reason } => {
                write!(f, "{} cannot be written: {reason}", path.display())
            }
        }
    }
}

// A run refused whole before any task starts, with every problem found.
#[derive(Debug, PartialEq)]
pub struct Refused {
    pub problems: Vec<Problem>,
}

impl From<Problem> for Refused {
    fn from(problem: Problem) -> Refused {
        Refused {
            problems: vec![problem],
        }
    }
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for problem in &self.problems {
            writeln!(f, "{problem}")?;
        }
        Ok(())
    }
}
