use std::time::Duration;

use crate::contract::Resources;

// What an attempt or a substep consumed as it measured itself: its wall, the
// CPU time of the thread that ran it, and every command it ran, each reaped
// with the kernel's account. A figure the host cannot give is absent, never
// zero.
impl Resources {
    // An absent message reads as protobuf reads it: every field at its default.
    pub(crate) fn unset() -> &'static Resources {
        static UNSET: Resources = Resources {
            wall_nanos: 0,
            cpu_nanos: None,
            processes: Vec::new(),
        };
        &UNSET
    }

    pub fn wall(&self) -> Duration {
        Duration::from_nanos(self.wall_nanos)
    }

    pub fn cpu(&self) -> Option<Duration> {
        self.cpu_nanos.map(Duration::from_nanos)
    }
}
