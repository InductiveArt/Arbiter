use std::path::Path;

use crate::wip::resources::Usage;
use crate::wip::run::refusal::Problem;

// How one attempt ended, as its medium saw it: how it was run, how it exited,
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

// Performs attempts. One is started in its folder, where its order lies and
// its product and record land; it is seen to have begun its body; it is asked
// to stop and then forced; and it is polled until it ended.
pub trait Medium {
    type Handle;
    // What stands between this medium and performing an attempt.
    fn verify(&self) -> Vec<Problem>;
    fn start(&mut self, dir: &Path) -> Result<Self::Handle, String>;
    fn started(&self, handle: &Self::Handle) -> bool;
    fn stop(&mut self, handle: &Self::Handle);
    fn force(&mut self, handle: &Self::Handle);
    fn poll(&mut self, handle: &mut Self::Handle) -> Option<Ended>;
}
