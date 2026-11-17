use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

use crate::domain::file::File;
use crate::domain::marker::Marker;

// One unit: its folder by path from the listed folder, empty for that folder
// itself, the markers it holds, and its files under the paths they were
// listed by.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Unit {
    pub path: PathBuf,
    pub markers: Vec<Marker>,
    pub files: Vec<File>,
}

// A unit as a person reads its place: its path, or a dot for the folder itself.
pub fn shown(unit: &Path) -> String {
    if unit.as_os_str().is_empty() {
        ".".to_string()
    } else {
        unit.to_string_lossy().into_owned()
    }
}
