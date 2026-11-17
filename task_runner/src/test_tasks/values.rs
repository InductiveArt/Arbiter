use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize)]
pub struct Root {
    pub name: String,
    pub parses: bool,
}

#[derive(Serialize, Deserialize)]
pub struct Repo {
    pub files: usize,
    pub parses: bool,
}

#[derive(Serialize, Deserialize)]
pub struct Graph {
    pub nodes: usize,
}

#[derive(Serialize, Deserialize)]
pub struct Cycles {
    pub found: usize,
}

#[derive(Serialize, Deserialize)]
pub struct Bounds {
    pub found: usize,
}

#[derive(Serialize, Deserialize)]
pub struct Tools {
    pub passed: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct Summary {
    pub line: String,
}

#[derive(Serialize, Deserialize)]
pub struct Licenses {
    pub bytes: usize,
}

#[derive(Serialize, Deserialize)]
pub struct Source {
    pub name: String,
    pub compiles: bool,
}

#[derive(Serialize, Deserialize)]
pub struct Patch {
    pub text: String,
    pub compiles: bool,
}

#[derive(Serialize, Deserialize)]
pub struct Checked;

#[derive(Serialize, Deserialize)]
pub struct Landed;

#[derive(Serialize, Deserialize)]
pub struct Reverted {
    pub text: String,
}

#[derive(Serialize, Deserialize)]
pub struct Archived;

#[derive(Serialize, Deserialize)]
pub struct Told {
    pub line: String,
}

#[derive(Serialize, Deserialize)]
pub struct Slow;

#[derive(Serialize, Deserialize)]
pub struct Later;

#[derive(Serialize, Deserialize)]
pub struct After;

#[derive(Serialize, Deserialize)]
pub struct Undone;

#[derive(Serialize, Deserialize)]
pub struct Said {
    pub line: String,
}

#[derive(Serialize, Deserialize)]
pub struct Boom;

#[derive(Serialize, Deserialize)]
pub struct Flakiness {
    pub passes_at: u32,
}

#[derive(Serialize, Deserialize)]
pub struct Flaked {
    pub attempt: u32,
}

#[derive(Serialize, Deserialize)]
pub struct Never;

#[derive(Serialize, Deserialize)]
pub struct Answered {
    pub line: String,
}

#[derive(Serialize, Deserialize)]
pub struct Slept;

#[derive(Serialize, Deserialize)]
pub struct Quick;

#[derive(Serialize, Deserialize)]
pub struct Turns {
    pub taken: u32,
}

#[derive(Serialize, Deserialize)]
pub struct Deaf;

#[derive(Serialize, Deserialize)]
pub struct Appetite {
    pub mebibytes: usize,
}

#[derive(Serialize, Deserialize)]
pub struct Held {
    pub bytes: usize,
}

#[derive(Serialize, Deserialize)]
pub struct Chatter {
    pub bytes: usize,
}

#[derive(Serialize, Deserialize)]
pub struct Printed {
    pub bytes: usize,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Costly {
    pub computed_at_nanos: u64,
}

#[derive(Serialize, Deserialize)]
pub struct Number {
    pub value: i64,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Squared {
    pub value: i64,
}

#[derive(Serialize, Deserialize)]
pub struct Total {
    pub value: i64,
}

#[derive(Serialize, Deserialize)]
pub struct Unsummed {
    pub reason: String,
}
