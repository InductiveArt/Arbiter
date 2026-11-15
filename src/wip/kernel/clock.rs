use std::time::Duration;

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
        let until = Instant::now() + ms(40);
        let mut x: u64 = 1;
        while Instant::now() < until {
            x = black_box(
                x.wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407),
            );
        }
        let spun = thread_cpu().expect("measured") - before;
        assert!(spun >= ms(10), "spun for 40ms of wall but {spun:?} of CPU");
        let before = thread_cpu().expect("measured");
        thread::sleep(ms(40));
        let slept = thread_cpu().expect("measured") - before;
        assert!(slept < ms(20), "sleeping cost {slept:?} of CPU");
    }
}
