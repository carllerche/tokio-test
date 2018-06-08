extern crate futures;

use futures::{future, Async};
use futures::executor::{spawn, Notify};

use std::sync::{Arc, Mutex, Condvar};
use std::sync::atomic::{AtomicUsize, Ordering};

/// TODO: Dox
#[derive(Debug)]
pub struct MockTask {
    notify: Arc<ThreadNotify>,
}

#[derive(Debug)]
struct ThreadNotify {
    state: AtomicUsize,
    mutex: Mutex<()>,
    condvar: Condvar,
}

const IDLE: usize = 0;
const NOTIFY: usize = 1;
const SLEEP: usize = 2;

impl MockTask {
    /// TODO: Dox
    pub fn new() -> Self {
        MockTask {
            notify: Arc::new(ThreadNotify::new()),
        }
    }

    pub fn enter<F, R>(&mut self, f: F) -> R
    where F: FnOnce() -> R,
    {
        self.notify.clear();

        let res = spawn(future::lazy(|| {
            Ok::<_, ()>(f())
        })).poll_future_notify(&self.notify, 0);

        match res.unwrap() {
            Async::Ready(v) => v,
            _ => unreachable!(),
        }
    }

    /// Returns `true` if the inner future has received a readiness notification
    /// since the last action has been performed.
    pub fn is_notified(&self) -> bool {
        self.notify.is_notified()
    }
}

impl ThreadNotify {
    fn new() -> Self {
        ThreadNotify {
            state: AtomicUsize::new(IDLE),
            mutex: Mutex::new(()),
            condvar: Condvar::new(),
        }
    }

    /// Clears any previously received notify, avoiding potential spurrious
    /// notifications. This should only be called immediately before running the
    /// task.
    fn clear(&self) {
        self.state.store(IDLE, Ordering::SeqCst);
    }

    fn is_notified(&self) -> bool {
        match self.state.load(Ordering::SeqCst) {
            IDLE => false,
            NOTIFY => true,
            _ => unreachable!(),
        }
    }
}

impl Notify for ThreadNotify {
    fn notify(&self, _unpark_id: usize) {
        // First, try transitioning from IDLE -> NOTIFY, this does not require a
        // lock.
        match self.state.compare_and_swap(IDLE, NOTIFY, Ordering::SeqCst) {
            IDLE | NOTIFY => return,
            SLEEP => {}
            _ => unreachable!(),
        }

        // The other half is sleeping, this requires a lock
        let _m = self.mutex.lock().unwrap();

        // Transition from SLEEP -> NOTIFY
        match self.state.compare_and_swap(SLEEP, NOTIFY, Ordering::SeqCst) {
            SLEEP => {}
            _ => return,
        }

        // Wakeup the sleeper
        self.condvar.notify_one();
    }
}
