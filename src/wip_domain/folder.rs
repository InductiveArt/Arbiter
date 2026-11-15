use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

// The folder a run is pointed at, by its absolute path: the process reading
// it runs elsewhere than whoever wrote the path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Folder {
    pub path: PathBuf,
}
