use std::time::Duration;

use crate::contract::Usage;

// The kernel's account of a process once it ended: its own figures and those
// of every descendant it waited for.
impl Usage {
    // An absent message reads as protobuf reads it: every field at its default.
    pub(crate) fn unset() -> &'static Usage {
        static UNSET: Usage = Usage {
            user_nanos: 0,
            system_nanos: 0,
            max_rss_bytes: 0,
            minor_faults: 0,
            major_faults: 0,
            in_blocks: 0,
            out_blocks: 0,
            voluntary_switches: 0,
            involuntary_switches: 0,
        };
        &UNSET
    }

    pub fn cpu(&self) -> Duration {
        Duration::from_nanos(self.user_nanos + self.system_nanos)
    }

    // Adds another process's account: times and counts sum, the peak keeps the
    // largest, since two processes never share one.
    pub fn add(&mut self, other: &Usage) {
        self.user_nanos += other.user_nanos;
        self.system_nanos += other.system_nanos;
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

    use crate::contract::Usage;

    fn ms(n: u64) -> u64 {
        n * 1_000_000
    }

    #[test]
    fn adding_accounts_sums_time_and_counts_and_keeps_the_largest_peak() {
        let mut total = Usage {
            user_nanos: ms(5),
            system_nanos: ms(1),
            max_rss_bytes: 4096,
            minor_faults: 10,
            ..Usage::default()
        };
        total.add(&Usage {
            user_nanos: ms(4),
            system_nanos: ms(3),
            max_rss_bytes: 8192,
            minor_faults: 5,
            major_faults: 1,
            ..Usage::default()
        });
        assert_eq!(
            (total.user_nanos, total.system_nanos, total.cpu()),
            (ms(9), ms(4), Duration::from_millis(13))
        );
        assert_eq!(total.max_rss_bytes, 8192);
        assert_eq!((total.minor_faults, total.major_faults), (15, 1));
    }
}
