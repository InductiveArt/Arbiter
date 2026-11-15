use std::io;
use std::process::Child;
use std::process::Command;
use std::process::ExitStatus;
use std::time::Duration;

use crate::wip::trace::account::Usage;

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

#[cfg(test)]
mod tests {
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
}
