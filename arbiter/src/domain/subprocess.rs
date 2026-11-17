use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

use crate::domain::ecosystem::Config;

// One program to start: the name its run goes by in the record, the program
// with the words handed to it, the folder it is started in, by its absolute
// path, the config files handed to it, written where a word holding `{pack}`
// points once it starts, and, when its run stands for a fact about the files
// of the unit it runs in, the digest of that unit's input cone, so a run over
// other files is another value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Subprocess {
    pub name: String,
    pub program: String,
    pub arguments: Vec<String>,
    pub folder: PathBuf,
    pub configs: Vec<Config>,
    pub cone: Option<String>,
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
    pub fn exited_with_zero(&self) -> bool {
        self.code == Some(0)
    }

    pub fn ended(&self, program: &str) -> String {
        match (self.code, self.signal) {
            (Some(code), _) => format!("{program} exited with {code}"),
            (None, Some(signal)) => format!("{program} was ended by signal {signal}"),
            (None, None) => format!("{program} ended with no exit code"),
        }
    }
}
