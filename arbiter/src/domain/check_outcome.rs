use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

// How one check ended in one unit: whether it passed, how long it took, all
// it told when it did not pass, and whether a stored green run answered for it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CheckOutcome {
    pub check: String,
    pub unit: PathBuf,
    pub language: String,
    pub passed: bool,
    pub seconds: f64,
    pub output: String,
    pub warm: bool,
}
