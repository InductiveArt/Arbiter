use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

static TERMINATED: AtomicBool = AtomicBool::new(false);

// Whether the host asked this process to stop.
pub fn terminated() -> bool {
    TERMINATED.load(Ordering::Relaxed)
}

// Makes the host's request to stop set a flag instead of ending the process
// at once, so a body stops at its next checkpoint and leaves its record.
#[cfg(unix)]
pub fn stop_on_terminate() {
    extern "C" fn noted(_: libc::c_int) {
        TERMINATED.store(true, Ordering::Relaxed);
    }
    let handler = noted as extern "C" fn(libc::c_int) as libc::sighandler_t;
    unsafe {
        libc::signal(libc::SIGTERM, handler);
    }
}

#[cfg(not(unix))]
pub fn stop_on_terminate() {}
