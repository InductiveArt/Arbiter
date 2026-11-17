use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

// The CPU time the calling thread has had so far.
#[cfg(unix)]
pub fn thread_cpu() -> Option<Duration> {
    let mut now: libc::timespec = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut now) };
    (rc == 0).then(|| {
        Duration::new(
            u64::try_from(now.tv_sec).unwrap_or(0),
            u32::try_from(now.tv_nsec).unwrap_or(0),
        )
    })
}

#[cfg(not(unix))]
pub fn thread_cpu() -> Option<Duration> {
    None
}

// A duration as the records count it, in nanoseconds; one past 584 years
// counts as the most there are.
pub fn nanos(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

// A time as the records count it, in nanoseconds since the Unix epoch,
// negative before it.
pub fn unix_nanos(time: SystemTime) -> i64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(after) => i64::try_from(after.as_nanos()).unwrap_or(i64::MAX),
        Err(before) => i64::try_from(before.duration().as_nanos()).map_or(i64::MIN, |gap| -gap),
    }
}

pub fn system_time(nanos: i64) -> SystemTime {
    let gap = Duration::from_nanos(nanos.unsigned_abs());
    if nanos < 0 {
        UNIX_EPOCH - gap
    } else {
        UNIX_EPOCH + gap
    }
}

#[cfg(test)]
mod tests {
    use std::hint::black_box;
    use std::thread;
    use std::time::Duration;
    use std::time::Instant;

    use super::thread_cpu;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn a_thread_s_cpu_time_grows_while_it_computes_and_not_while_it_sleeps() {
        let before = thread_cpu().expect("thread CPU time is measured on this host");
        // Computes until the thread has had 20ms of CPU, however long a loaded
        // machine takes to give them, within five seconds of wall.
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut x: u64 = 1;
        while thread_cpu().is_some_and(|now| now.saturating_sub(before) < Duration::from_millis(20))
            && Instant::now() < deadline
        {
            for _ in 0..10_000 {
                x = black_box(
                    x.wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407),
                );
            }
        }
        let spun = thread_cpu().expect("measured") - before;
        assert!(
            spun >= ms(20),
            "five seconds of computing moved the thread's clock {spun:?}"
        );
        let before = thread_cpu().expect("measured");
        thread::sleep(ms(40));
        let slept = thread_cpu().expect("measured") - before;
        assert!(slept < ms(20), "sleeping cost {slept:?} of CPU");
    }
}
