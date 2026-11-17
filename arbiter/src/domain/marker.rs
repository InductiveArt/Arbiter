use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

// One listed file that makes its folder a unit: its path as listed, the
// ecosystem it marks for, and whether it only lists member units.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Marker {
    pub path: PathBuf,
    pub ecosystem: String,
    pub members_only: bool,
}
