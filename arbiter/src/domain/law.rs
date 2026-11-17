use serde::Deserialize;
use serde::Serialize;

use crate::domain::finding::Finding;

// A law by its name: one of the core's laws, or one language's laws by the
// language's own name.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LawName {
    pub name: String,
}

// What one law found, held over the graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LawFindings {
    pub law: String,
    pub findings: Vec<Finding>,
}
