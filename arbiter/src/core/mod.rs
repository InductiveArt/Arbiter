pub mod digest;

// The one version the whole binary stamps: the gate's snapshot and the adopted
// files both name this. It sits in core because every
// plane that stamps it already stands on core.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod declarations;
pub mod definition;
pub mod drift;
pub mod ecosystems;
pub mod edge;
pub mod graph;
pub mod graph_data;
pub mod language;
pub mod listing;
pub mod markers;
pub mod measures;
pub mod pack;
pub mod prose;
pub mod reader;
pub mod record;
pub mod repository;
pub mod rule;
pub mod stance;
pub mod suppression;
pub mod task;
pub mod tools;
pub mod trees;
pub mod units;
pub mod warm;
