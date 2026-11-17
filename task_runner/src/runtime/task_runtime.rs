use std::path::Path;

use crate::record::refusal::RefusalReason;
use crate::runtime::ended::Ended;

// Performs attempts. One is started in its folder, where its order lies and
// its output and record land; it is seen to have begun its body; it is asked
// to stop and then forced; and it is polled until it ended.
pub trait TaskRuntime {
    type TaskHandle;
    // What stands between this runtime and performing an attempt.
    fn verify(&self) -> Vec<RefusalReason>;
    fn start(&mut self, dir: &Path) -> Result<Self::TaskHandle, String>;
    fn started(&self, handle: &Self::TaskHandle) -> bool;
    fn stop(&mut self, handle: &Self::TaskHandle);
    fn force(&mut self, handle: &Self::TaskHandle);
    fn poll(&mut self, handle: &mut Self::TaskHandle) -> Option<Ended>;
}
