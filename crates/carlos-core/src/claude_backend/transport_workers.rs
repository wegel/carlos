//! Cancels queue waits without waiting for workers that may still hold inherited pipes.

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

/// Shares cancellation with every worker in one helper generation.
#[derive(Default)]
pub(super) struct TransportWorkers {
    cancelled: AtomicBool,
}

impl TransportWorkers {
    /// Detach a worker, as main does, because another process may keep its pipe open.
    pub(super) fn spawn(&self, work: impl FnOnce() + Send + 'static) {
        thread::spawn(work);
    }

    /// Test whether client teardown has cancelled this generation's queue waits.
    pub(super) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Release final-event senders without joining readers, writers, or their follow-up workers.
    pub(super) fn close(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
}
