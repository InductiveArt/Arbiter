// How a root is laid out on disk: one folder per run, and the store of
// products kept across runs, each under its key.
pub const RUNS_DIR: &str = "runs";
pub const STORE_DIR: &str = "store";
// Inside a run: the seeds as files, the pid of the run while it goes, and the
// whole report once it ended. Inside an attempt: the key its product is
// stored under, and what the process printed.
pub const SEEDS_DIR: &str = "seeds";
pub const RUNNING_FILE: &str = "running";
pub const RUN_FILE: &str = "run.json";
pub const KEY_FILE: &str = "product.key";
pub const STDOUT_FILE: &str = "stdout";
pub const STDERR_FILE: &str = "stderr";
