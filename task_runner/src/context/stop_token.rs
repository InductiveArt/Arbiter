use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;

// A flag one side sets and every clone reads.
#[derive(Clone, Default)]
pub struct StopToken {
    set: Arc<AtomicBool>,
}

impl StopToken {
    pub fn new() -> StopToken {
        StopToken::default()
    }

    pub fn stopped(&self) -> bool {
        self.set.load(Ordering::SeqCst)
    }

    pub fn stop(&self) {
        self.set.store(true, Ordering::SeqCst);
    }
}
