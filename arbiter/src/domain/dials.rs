use serde::Deserialize;
use serde::Serialize;

use crate::domain::finding::Finding;

// What every unit's dials read: each dial unset or lowered, as a finding, and
// what the languages told about the machine while reading them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Dials {
    pub findings: Vec<Finding>,
    pub notes: Vec<String>,
}
