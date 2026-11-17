use serde::Deserialize;
use serde::Serialize;

use crate::domain::check_outcome::CheckOutcome;

// How every unit's generated code compared with its contracts, and what the
// languages told while comparing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeneratedChecks {
    pub outcomes: Vec<CheckOutcome>,
    pub notes: Vec<String>,
}
