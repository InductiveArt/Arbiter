// Tasks wired by the types they read and produce, planned into one order, each
// attempt run as a process of its own and reaped with the kernel's account of
// it, every record kept on disk under its run. The surface is what a user
// needs to declare tasks, run a plan and read what a run left; the rest is
// the crate's own.
pub mod attempt;
pub mod context;
// The generator's serde code borrows into a format where clippy wants no
// borrow, and generated code is never edited by hand.
#[allow(clippy::useless_borrows_in_formatting)]
pub mod contract;
pub mod declaration;
pub mod execution;
pub mod kernel;
pub mod protocol;
pub mod record;
pub(crate) mod runtime;
#[cfg(feature = "test-tasks")]
pub mod test_tasks;
pub mod value;
