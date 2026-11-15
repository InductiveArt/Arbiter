use std::time::Duration;

use serde::Deserialize;
use serde::Serialize;

// What one frame consumed as it measured itself: its wall, the CPU time of the
// thread that ran it, and every command it ran, each reaped with the kernel's
// account. A figure the host cannot give is absent, never zero.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Resources {
    pub wall: Duration,
    pub cpu: Option<Duration>,
    pub children: Vec<Account>,
}

// One process as the one that ran it saw it: what ran, how long, how long it
// took to come up where that was marked, how it ended, whether it was stopped
// from outside, how much it printed and how much of that was kept, and the
// kernel's account of it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Account {
    pub program: String,
    pub args: Vec<String>,
    pub wall: Duration,
    pub startup: Option<Duration>,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
    pub killed: bool,
    pub stdout_bytes: u64,
    pub stdout_kept: u64,
    pub stderr_bytes: u64,
    pub stderr_kept: u64,
    pub usage: Usage,
}

// The kernel's account of a process once it ended: its own figures and those
// of every descendant it waited for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub user: Duration,
    pub system: Duration,
    pub max_rss_bytes: u64,
    pub minor_faults: u64,
    pub major_faults: u64,
    pub in_blocks: u64,
    pub out_blocks: u64,
    pub voluntary_switches: u64,
    pub involuntary_switches: u64,
}

impl Usage {
    pub fn cpu(&self) -> Duration {
        self.user + self.system
    }

    // Adds another process's account: times and counts sum, the peak keeps the
    // largest, since two processes never share one.
    pub fn add(&mut self, other: &Usage) {
        self.user += other.user;
        self.system += other.system;
        self.max_rss_bytes = self.max_rss_bytes.max(other.max_rss_bytes);
        self.minor_faults += other.minor_faults;
        self.major_faults += other.major_faults;
        self.in_blocks += other.in_blocks;
        self.out_blocks += other.out_blocks;
        self.voluntary_switches += other.voluntary_switches;
        self.involuntary_switches += other.involuntary_switches;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::Usage;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn adding_accounts_sums_time_and_counts_and_keeps_the_largest_peak() {
        let mut total = Usage {
            user: ms(5),
            system: ms(1),
            max_rss_bytes: 4096,
            minor_faults: 10,
            ..Usage::default()
        };
        total.add(&Usage {
            user: ms(4),
            system: ms(3),
            max_rss_bytes: 8192,
            minor_faults: 5,
            major_faults: 1,
            ..Usage::default()
        });
        assert_eq!(
            (total.user, total.system, total.cpu()),
            (ms(9), ms(4), ms(13))
        );
        assert_eq!(total.max_rss_bytes, 8192);
        assert_eq!((total.minor_faults, total.major_faults), (15, 1));
    }
}
