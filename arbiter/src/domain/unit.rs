use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

use crate::domain::file::File;
use crate::domain::marker::Marker;

// One unit: its folder by path from the listed folder, empty for that folder
// itself, the markers it holds, and its files under the paths they were
// listed by.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Unit {
    pub path: PathBuf,
    pub markers: Vec<Marker>,
    pub files: Vec<File>,
}
