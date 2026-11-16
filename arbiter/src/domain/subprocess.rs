use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

// One program to start: the name its run goes by in the record, the program
// with the words handed to it, and the folder it is started in, by its
// absolute path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Subprocess {
    pub name: String,
    pub program: String,
    pub arguments: Vec<String>,
    pub folder: PathBuf,
}

// How one program ended and all it printed, held as text so the value reads
// back whole wherever it is handed; a byte that is not UTF-8 reads as the
// replacement character.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubprocessExit {
    pub code: Option<i32>,
    pub signal: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl SubprocessExit {
    pub fn ended(&self, program: &str) -> String {
        match (self.code, self.signal) {
            (Some(code), _) => format!("{program} exited with {code}"),
            (None, Some(signal)) => format!("{program} was ended by signal {signal}"),
            (None, None) => format!("{program} ended with no exit code"),
        }
    }
}
