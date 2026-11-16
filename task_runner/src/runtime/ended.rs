use crate::contract::Usage;

// How one attempt ended, as its runtime saw it: how it was run, how it exited,
// the kernel's account, how many bytes it printed and how many were kept, and
// what went wrong in watching it, if anything.
pub struct Ended {
    pub program: String,
    pub args: Vec<String>,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
    pub usage: Usage,
    pub stdout: (u64, u64),
    pub stderr: (u64, u64),
    pub fault: Option<String>,
}
