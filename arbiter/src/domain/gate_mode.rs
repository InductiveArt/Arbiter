use serde::Deserialize;
use serde::Serialize;

// How a gate runs: whether it repairs what it can before it checks, and
// whether it reads nothing a past run stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateMode {
    pub fixing: bool,
    pub cold: bool,
}
