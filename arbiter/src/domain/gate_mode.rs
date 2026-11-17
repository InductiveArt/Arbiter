use serde::Deserialize;
use serde::Serialize;

// How a gate runs: whether it reads nothing a past run stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateMode {
    pub cold: bool,
}
