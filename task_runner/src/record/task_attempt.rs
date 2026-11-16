use std::fs;
use std::io;
use std::path::Path;
use std::time::Duration;
use std::time::SystemTime;

use crate::contract::Resources;
use crate::contract::RunRecord;
use crate::contract::TaskAttempt;
use crate::contract::TaskAttemptId;
use crate::kernel::clock::system_time;
use crate::value::task_cache_key::TaskCacheKey;
use crate::value::task_output_hash::TaskOutputHash;

// One try at an invocation: how it ended, when it began, the slot that ran it
// and how long it waited for one, whether its output came from the store
// instead, the output it left by hash and the key it is cached under, what it
// measured of itself from inside, its own process as the run saw it, what its
// body counted and noted, and the substeps it ran.
impl TaskAttempt {
    // The record of an invocation decided without a try, or of a try that
    // left no record of its own.
    pub fn unrun(id: TaskAttemptId, outcome: RunRecord) -> TaskAttempt {
        TaskAttempt {
            id: Some(id),
            outcome: Some(outcome),
            ..TaskAttempt::default()
        }
    }

    pub fn id(&self) -> &TaskAttemptId {
        self.id.as_ref().unwrap_or(TaskAttemptId::unset())
    }

    pub fn outcome(&self) -> &RunRecord {
        self.outcome.as_ref().unwrap_or(RunRecord::unset())
    }

    pub fn began(&self) -> Option<SystemTime> {
        self.began_unix_nanos.map(system_time)
    }

    pub fn wait(&self) -> Option<Duration> {
        self.wait_nanos.map(Duration::from_nanos)
    }

    // The output the attempt left and the key it is cached under; a name
    // that is no hash reads as none, since nothing is kept under it.
    pub fn output_hash(&self) -> Option<TaskOutputHash> {
        self.output.as_deref().and_then(TaskOutputHash::parse)
    }

    pub fn task_cache_key(&self) -> Option<TaskCacheKey> {
        self.cache_key.as_deref().and_then(TaskCacheKey::parse)
    }

    pub fn resources(&self) -> &Resources {
        self.resources.as_ref().unwrap_or(Resources::unset())
    }

    pub fn seconds(&self) -> f64 {
        self.resources().wall().as_secs_f64()
    }

    // The record in protobuf's JSON form, which any language reads.
    pub fn write(&self, file: &Path) -> io::Result<()> {
        let bytes = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        fs::write(file, bytes)
    }

    // A record that does not read as the contract is refused as invalid data.
    pub fn read(file: &Path) -> io::Result<TaskAttempt> {
        serde_json::from_slice(&fs::read(file)?)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }
}
