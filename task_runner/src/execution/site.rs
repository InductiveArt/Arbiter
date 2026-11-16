use std::path::PathBuf;

// Where a run lives on disk: its own folder, and the root holding every run,
// the outputs and the cache.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Site {
    pub dir: PathBuf,
    pub root: PathBuf,
}
