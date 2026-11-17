use serde::Deserialize;
use serde::Serialize;

use crate::domain::unit_check::UnitCheck;

// Every check planned, in the order planned, and a note for each unit held by
// an ecosystem that plans nothing in it: a marker only listing members, or an
// ecosystem with no tool.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlannedChecks {
    pub checks: Vec<UnitCheck>,
    pub notes: Vec<String>,
}
