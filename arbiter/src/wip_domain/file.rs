use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct File {
    pub path: PathBuf,
    pub bytes: u64,
    pub digest: String,
}
