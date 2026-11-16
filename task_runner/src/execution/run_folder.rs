// How a root is laid out on disk: one folder per run, the outputs tasks left
// kept across runs, and the cache naming them.
pub const RUNS_DIR: &str = "runs";
pub const OUTPUTS_DIR: &str = "outputs";
pub(crate) const CACHE_DIR: &str = "cache";
// Inside a run: the seeds as files, the pid of the run while it goes, and the
// whole report once it ended.
pub(crate) const SEEDS_DIR: &str = "seeds";
pub const RUNNING_FILE: &str = "running";
pub const RUN_FILE: &str = "run.json";
