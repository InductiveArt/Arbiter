use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

// One thing a check found: the check that found it, the rule it breaks as that
// check names it, the file or the unit it is in by path from the listed
// folder, and its line when it has one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    pub check: String,
    pub rule: Option<String>,
    pub path: PathBuf,
    pub line: Option<u32>,
    pub message: String,
}
