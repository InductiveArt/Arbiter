use std::path::PathBuf;

// A directive in the code that quiets a rule, as the ledger counts it.
#[derive(Clone)]
pub struct Suppression {
    pub file: PathBuf,
    pub line: usize,
    // The directive as written, which with the file is the entry's identity:
    // a line number moves with every edit above it, so it never identifies.
    pub text: String,
    // Whether it quiets more than one named rule, which is refused, never recorded.
    pub blanket: bool,
}
