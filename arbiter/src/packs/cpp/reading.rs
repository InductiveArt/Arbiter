use std::path::PathBuf;

// What the C++ pack read from one file: the headers it includes and the
// names it calls.
pub struct Reading {
    pub file: PathBuf,
    pub includes: Vec<String>,
    pub calls: Vec<Call>,
}

pub struct Call {
    pub name: String,
    pub line: usize,
}
