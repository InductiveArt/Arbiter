use std::any::Any;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::job::id::RunId;
use crate::store::Store;
use crate::trace::account::Usage;
use crate::trace::outcome::Outcome;
use crate::trace::step::Step;

// How a run ended, the kernel's account of every attempt of its own tasks
// summed, where it lives on disk, what it left in the store, and every
// attempt's record, task by task in plan order.
pub struct Report {
    pub run: RunId,
    pub outcome: Outcome,
    pub seconds: f64,
    pub usage: Usage,
    pub dir: PathBuf,
    pub store: Store,
    pub names: Vec<String>,
    pub attempts: Vec<Vec<Step>>,
}

impl Report {
    pub fn get<T: DeserializeOwned + Any>(&self) -> Option<T> {
        self.store.get::<T>()
    }

    pub fn attempts_of(&self, name: &str) -> &[Step] {
        self.names
            .iter()
            .position(|known| known == name)
            .map_or(&[], |at| &self.attempts[at])
    }

    // The last attempt's record: how the task ended.
    pub fn step(&self, name: &str) -> Option<&Step> {
        self.attempts_of(name).last()
    }

    // Every task's last record, in plan order.
    pub fn steps(&self) -> Vec<&Step> {
        self.attempts
            .iter()
            .filter_map(|made| made.last())
            .collect()
    }

    pub fn write(&self, file: &Path) -> io::Result<()> {
        let written = Written {
            run: self.run.to_string(),
            outcome: &self.outcome,
            seconds: self.seconds,
            usage: &self.usage,
            tasks: self
                .names
                .iter()
                .zip(&self.attempts)
                .map(|(name, attempts)| WrittenTask { name, attempts })
                .collect(),
        };
        let bytes = serde_json::to_vec_pretty(&written).map_err(io::Error::other)?;
        fs::write(file, bytes)
    }
}

#[derive(Serialize)]
struct Written<'a> {
    run: String,
    outcome: &'a Outcome,
    seconds: f64,
    usage: &'a Usage,
    tasks: Vec<WrittenTask<'a>>,
}

#[derive(Serialize)]
struct WrittenTask<'a> {
    name: &'a str,
    attempts: &'a [Step],
}
