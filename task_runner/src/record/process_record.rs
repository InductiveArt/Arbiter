use std::time::Duration;

use crate::contract::ProcessRecord;
use crate::contract::Usage;

// One process as the one that ran it saw it: what ran, how long, how long it
// took to come up where that was marked, how it ended, whether it was stopped
// from outside, how much it printed and how much of that was kept, and the
// kernel's account of it.
impl ProcessRecord {
    pub fn wall(&self) -> Duration {
        Duration::from_nanos(self.wall_nanos)
    }

    pub fn startup(&self) -> Option<Duration> {
        self.startup_nanos.map(Duration::from_nanos)
    }

    pub fn usage(&self) -> &Usage {
        self.usage.as_ref().unwrap_or(Usage::unset())
    }
}
