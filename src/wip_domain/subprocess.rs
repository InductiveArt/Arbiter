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

// How one program ended, and the files holding all it printed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Exit {
    pub code: Option<i32>,
    pub signal: Option<i32>,
    pub stdout: PathBuf,
    pub stderr: PathBuf,
}

impl Exit {
    pub fn ended(&self, program: &str) -> String {
        match (self.code, self.signal) {
            (Some(code), _) => format!("{program} exited with {code}"),
            (None, Some(signal)) => format!("{program} was ended by signal {signal}"),
            (None, None) => format!("{program} ended with no exit code"),
        }
    }
}
