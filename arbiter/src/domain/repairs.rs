use serde::Deserialize;
use serde::Serialize;

use crate::domain::check_outcome::CheckOutcome;

// What a fixing run repaired before the gate: a note for each file a language
// rewrote, and the outcome of each repair that did not succeed, which stands
// for its check in the same unit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Repairs {
    pub notes: Vec<String>,
    pub failed: Vec<CheckOutcome>,
}
