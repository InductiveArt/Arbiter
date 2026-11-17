use std::path::PathBuf;

// One thing a tool printed about the code, in the tool's own terms: the file
// as the tool writes it, the line, the tool's own code for it, and its words.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Diagnostic {
    pub file: PathBuf,
    pub line: Option<u32>,
    pub code: Option<String>,
    pub message: String,
}
