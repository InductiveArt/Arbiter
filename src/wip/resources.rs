use std::io;
use std::process::Child;
use std::process::Command;
use std::process::ExitStatus;
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

// A child that ended: its status, the same decoded, and its account.
pub struct Reaped {
    pub status: ExitStatus,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
    pub usage: Usage,
}

fn pid_of(child: &Child) -> io::Result<libc::pid_t> {
    libc::pid_t::try_from(child.id())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "pid out of range"))
}

// Reaps the child when it has ended, or at once when told to wait. Nothing
// yet when it is still running and not waited for. The child is gone from
// the host once reaped, so its own wait fails after.
#[cfg(unix)]
pub fn reap(child: &mut Child, wait: bool) -> io::Result<Option<Reaped>> {
    use std::os::unix::process::ExitStatusExt;

    let pid = pid_of(child)?;
    let mut status: libc::c_int = 0;
    let mut rusage: libc::rusage = unsafe { std::mem::zeroed() };
    let options = if wait { 0 } else { libc::WNOHANG };
    let reaped = unsafe { libc::wait4(pid, &mut status, options, &mut rusage) };
    if reaped == -1 {
        return Err(io::Error::last_os_error());
    }
    if reaped == 0 {
        return Ok(None);
    }
    let status = ExitStatus::from_raw(status);
    Ok(Some(Reaped {
        exit_code: status.code(),
        signal: status.signal(),
        usage: usage_of(&rusage),
        status,
    }))
}

#[cfg(not(unix))]
pub fn reap(_child: &mut Child, _wait: bool) -> io::Result<Option<Reaped>> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "reaping a child with its account needs a unix host",
    ))
}

// Whether the child has ended, without reaping it, so its pid is still held
// and a signal to its group still reaches only its own.
#[cfg(any(target_vendor = "apple", target_os = "linux"))]
pub fn exited(child: &Child) -> io::Result<bool> {
    let pid = pid_of(child)?;
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    let options = libc::WEXITED | libc::WNOHANG | libc::WNOWAIT;
    let rc = unsafe { libc::waitid(libc::P_PID, child.id(), &mut info, options) };
    if rc == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(reported(&info) == pid)
}

#[cfg(not(any(target_vendor = "apple", target_os = "linux")))]
pub fn exited(_child: &Child) -> io::Result<bool> {
    Ok(false)
}

#[cfg(target_vendor = "apple")]
fn reported(info: &libc::siginfo_t) -> libc::pid_t {
    info.si_pid
}

#[cfg(target_os = "linux")]
fn reported(info: &libc::siginfo_t) -> libc::pid_t {
    unsafe { info.si_pid() }
}

// Puts the command's process in a group of its own, so a signal to the group
// reaches it and everything it starts, and nothing else.
pub fn own_group(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(not(unix))]
    let _ = command;
}

// Asks the child to stop: the whole group it leads, or the one process.
pub fn terminate(child: &Child, group: bool) -> io::Result<()> {
    signal(child, group, libc::SIGTERM)
}

// Ends the child at once: the whole group it leads, or the one process.
pub fn kill(child: &Child, group: bool) -> io::Result<()> {
    signal(child, group, libc::SIGKILL)
}

#[cfg(unix)]
fn signal(child: &Child, group: bool, signal: libc::c_int) -> io::Result<()> {
    let pid = pid_of(child)?;
    let target = if group { -pid } else { pid };
    let rc = unsafe { libc::kill(target, signal) };
    if rc == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(unix))]
fn signal(_child: &Child, _group: bool, _signal: libc::c_int) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "signalling a process needs a unix host",
    ))
}

// Whether a process with this id is on the host now.
#[cfg(unix)]
pub fn alive(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    let rc = unsafe { libc::kill(pid, 0) };
    rc == 0 || io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(not(unix))]
pub fn alive(_pid: u32) -> bool {
    false
}

#[cfg(unix)]
fn usage_of(rusage: &libc::rusage) -> Usage {
    Usage {
        user: timeval(rusage.ru_utime),
        system: timeval(rusage.ru_stime),
        max_rss_bytes: rss_bytes(rusage.ru_maxrss),
        minor_faults: count(rusage.ru_minflt),
        major_faults: count(rusage.ru_majflt),
        in_blocks: count(rusage.ru_inblock),
        out_blocks: count(rusage.ru_oublock),
        voluntary_switches: count(rusage.ru_nvcsw),
        involuntary_switches: count(rusage.ru_nivcsw),
    }
}

#[cfg(unix)]
fn timeval(time: libc::timeval) -> Duration {
    let micros = u32::try_from(time.tv_usec).unwrap_or(0);
    Duration::new(u64::try_from(time.tv_sec).unwrap_or(0), micros * 1000)
}

#[cfg(unix)]
fn count(n: libc::c_long) -> u64 {
    u64::try_from(n).unwrap_or(0)
}

// The kernel reports the peak in bytes on Apple hosts and kilobytes elsewhere.
#[cfg(all(unix, target_vendor = "apple"))]
fn rss_bytes(n: libc::c_long) -> u64 {
    count(n)
}

#[cfg(all(unix, not(target_vendor = "apple")))]
fn rss_bytes(n: libc::c_long) -> u64 {
    count(n) * 1024
}

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
    use std::process::Command;
    use std::thread;
    use std::time::Duration;
    use std::time::Instant;

    use super::alive;
    use super::exited;
    use super::kill;
    use super::own_group;
    use super::reap;
    use super::terminate;
    use super::thread_cpu;
    use super::Usage;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn a_reaped_child_reports_how_it_ended_and_what_it_used() {
        let mut child = Command::new("sh")
            .args(["-c", "exit 3"])
            .spawn()
            .expect("sh spawns");
        let reaped = loop {
            match reap(&mut child, false).expect("wait4 works") {
                Some(reaped) => break reaped,
                None => thread::sleep(ms(2)),
            }
        };
        assert_eq!(reaped.exit_code, Some(3));
        assert_eq!(reaped.signal, None);
        assert!(!reaped.status.success());
        assert!(
            reaped.usage.max_rss_bytes > 0,
            "a child's peak memory is accounted"
        );
    }

    #[test]
    fn a_running_child_reaps_as_nothing_yet_and_a_killed_one_as_its_signal() {
        let mut child = Command::new("sleep")
            .arg("5")
            .spawn()
            .expect("sleep spawns");
        assert!(reap(&mut child, false).expect("wait4 works").is_none());
        assert!(!exited(&child).expect("waitid works"));
        child.kill().expect("killable");
        let reaped = reap(&mut child, true).expect("wait4 works").expect("ended");
        assert_eq!(reaped.signal, Some(9));
        assert_eq!(reaped.exit_code, None);
    }

    #[test]
    fn an_ended_child_is_seen_as_exited_before_it_is_reaped() {
        let mut child = Command::new("true").spawn().expect("true spawns");
        let pid = child.id();
        let seen = loop {
            if exited(&child).expect("waitid works") {
                break true;
            }
            thread::sleep(ms(2));
        };
        assert!(seen);
        assert!(alive(pid), "an unreaped child still holds its pid");
        let reaped = reap(&mut child, true).expect("wait4 works").expect("ended");
        assert_eq!(reaped.exit_code, Some(0));
    }

    #[test]
    fn a_signal_to_a_group_ends_the_leader_and_what_it_started() {
        let mut command = Command::new("sh");
        command.args(["-c", "sleep 5 & sleep 5; wait"]);
        own_group(&mut command);
        let mut child = command.spawn().expect("sh spawns");
        thread::sleep(ms(50));
        terminate(&child, true).expect("the group is signalled");
        let begun = Instant::now();
        let reaped = reap(&mut child, true).expect("wait4 works").expect("ended");
        assert_eq!(reaped.signal, Some(15));
        assert!(begun.elapsed() < Duration::from_secs(2));
        let _ = kill(&child, true);
        let _ = kill(&child, false);
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
