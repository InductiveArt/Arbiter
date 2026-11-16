use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

use crate::domain::ecosystem::Reader;
use crate::domain::subprocess::Subprocess;

// One tool to run in one unit: the tool by name, the unit by its path from
// the listed folder, the program running the tool there, and how what it
// prints is read into findings when it is.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UnitCheck {
    pub check: String,
    pub unit: PathBuf,
    pub subprocess: Subprocess,
    pub reading: Option<CheckReading>,
}

// The reader of what a check prints, and the program printing the root the
// paths it prints run from, when that is not the folder the check runs in.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CheckReading {
    pub reader: Reader,
    pub root: Option<Subprocess>,
}

impl UnitCheck {
    // The programs the check runs, in the order they are listed: its own,
    // then the one printing its root when it has one.
    pub fn subprocesses(&self) -> Vec<&Subprocess> {
        let mut listed = vec![&self.subprocess];
        listed.extend(
            self.reading
                .as_ref()
                .and_then(|reading| reading.root.as_ref()),
        );
        listed
    }
}
