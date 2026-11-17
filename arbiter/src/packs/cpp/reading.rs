use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

// What the C++ pack read from one file: the headers it includes and the
// names it calls.
#[derive(Serialize, Deserialize)]
pub struct Reading {
    pub file: PathBuf,
    pub includes: Vec<String>,
    pub calls: Vec<Call>,
}

#[derive(Serialize, Deserialize)]
pub struct Call {
    pub name: String,
    pub line: usize,
}
