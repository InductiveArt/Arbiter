use std::time::SystemTime;

use crate::contract::Resources;
use crate::contract::RunRecord;
use crate::contract::TaskSubstep;
use crate::kernel::clock::system_time;

// One named part of an attempt's work, run inside the attempt on its thread,
// so its figures are already inside the attempt's: how it ended, when it
// began, what it measured of itself, what it counted and noted, and the
// substeps it ran.
impl TaskSubstep {
    pub fn outcome(&self) -> &RunRecord {
        self.outcome.as_ref().unwrap_or(RunRecord::unset())
    }

    pub fn began(&self) -> Option<SystemTime> {
        self.began_unix_nanos.map(system_time)
    }

    pub fn resources(&self) -> &Resources {
        self.resources.as_ref().unwrap_or(Resources::unset())
    }

    pub fn seconds(&self) -> f64 {
        self.resources().wall().as_secs_f64()
    }
}
