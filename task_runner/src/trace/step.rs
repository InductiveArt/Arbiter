use std::collections::BTreeMap;
use std::time::Duration;
use std::time::SystemTime;

use serde::Deserialize;
use serde::Serialize;

use crate::job::id::JobId;
use crate::trace::account::Account;
use crate::trace::account::Resources;
use crate::trace::outcome::Outcome;

// How a record relates to the one holding it. An attempt ran as a process of
// its own and is accounted on its own; an inner frame ran inside its parent,
// on the parent's thread, so its figures are already inside the parent's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Frame {
    Attempt,
    Inner,
}

// One unit of work that ran, with the units it ran inside itself. The job
// names the run, the path from the root and the attempt, so a unit is found by
// name at any depth and one try is told from the next. Plain data throughout,
// so a record is written and read back whole.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Step {
    pub job: JobId,
    pub name: String,
    pub outcome: Outcome,
    pub frame: Frame,
    pub began: Option<SystemTime>,
    // The slot that ran the attempt, and how long it waited for one.
    pub slot: Option<usize>,
    pub wait: Option<Duration>,
    // The product was taken from the store; no process ran.
    pub replayed: bool,
    // What the frame measured of itself, from inside.
    pub resources: Resources,
    // The attempt's own process, as the run that spawned it saw it.
    pub account: Option<Account>,
    // Quantities the body itself counted, by name.
    pub measures: BTreeMap<String, f64>,
    pub evidence: Vec<String>,
    pub children: Vec<Step>,
}

impl Step {
    // The record of a task that never ran.
    pub fn unrun(job: JobId, name: &str, outcome: Outcome) -> Step {
        Step {
            job,
            name: name.to_string(),
            outcome,
            frame: Frame::Attempt,
            began: None,
            slot: None,
            wait: None,
            replayed: false,
            resources: Resources::default(),
            account: None,
            measures: BTreeMap::new(),
            evidence: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn seconds(&self) -> f64 {
        self.resources.wall.as_secs_f64()
    }
}
