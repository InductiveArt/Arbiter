use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;

// A token one side sets and the other reads. Whoever asks to be woken is
// called once, under the same lock that sets the flag, so no asker is missed.
#[derive(Clone, Default)]
pub struct Cancel {
    inner: Arc<Flag>,
}

#[derive(Default)]
struct Flag {
    set: AtomicBool,
    wakers: Mutex<Vec<Box<dyn Fn() + Send + Sync>>>,
}

impl Cancel {
    pub fn new() -> Cancel {
        Cancel::default()
    }

    pub fn cancelled(&self) -> bool {
        self.inner.set.load(Ordering::SeqCst)
    }

    pub fn cancel(&self) {
        let drained: Vec<Box<dyn Fn() + Send + Sync>> = {
            let mut wakers = self
                .inner
                .wakers
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            self.inner.set.store(true, Ordering::SeqCst);
            wakers.drain(..).collect()
        };
        for waker in drained {
            waker();
        }
    }

    pub fn on_cancel(&self, waker: impl Fn() + Send + Sync + 'static) {
        let mut wakers = self
            .inner
            .wakers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if self.cancelled() {
            drop(wakers);
            waker();
        } else {
            wakers.push(Box::new(waker));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;

    use super::Cancel;

    #[test]
    fn a_cancel_token_wakes_each_asker_once_and_a_late_asker_at_once() {
        let cancel = Cancel::new();
        let woken = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&woken);
        cancel.on_cancel(move || {
            count.fetch_add(1, Ordering::SeqCst);
        });
        assert!(!cancel.cancelled());
        cancel.cancel();
        cancel.cancel();
        assert!(cancel.cancelled());
        assert_eq!(woken.load(Ordering::SeqCst), 1);
        let late = Arc::clone(&woken);
        cancel.on_cancel(move || {
            late.fetch_add(10, Ordering::SeqCst);
        });
        assert_eq!(woken.load(Ordering::SeqCst), 11);
    }
}
