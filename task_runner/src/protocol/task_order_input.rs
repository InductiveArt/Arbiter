use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

use crate::declaration::read::When;
use crate::record::completion::Completion;

// One read as an order hands it: the name of its value's type, when it fires,
// and the file to decode, the completion, or both.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct TaskOrderInput {
    pub value_type: String,
    pub when: When,
    pub value: Option<PathBuf>,
    pub completion: Option<Completion>,
}
