pub mod claims;
pub mod digest;

// The one version the whole binary stamps: the gate's snapshot, the examiner's
// receipts, and the adopted files all name this. It sits in core because every
// plane that stamps it already stands on core.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod graph;
pub mod pack;
pub mod prose;
pub mod record;
pub mod trace;
pub mod trees;
pub mod units;
pub mod warm;
