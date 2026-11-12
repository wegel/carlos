//! Keeps a batch slot owned until synchronous or deferred queue delivery finishes.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};

use anyhow::{anyhow, Result};

use super::transport_workers::TransportWorkers;

#[derive(Default)]
struct BatchState {
    busy: bool,
    terminal: bool,
    deferred: VecDeque<(usize, Box<dyn FnOnce(BatchPermit) + Send>)>,
    deferred_bytes: usize,
}

/// Orders producers without holding a mutex guard across any queue send.
#[derive(Clone)]
pub(super) struct BatchGate(Arc<(Mutex<BatchState>, Condvar)>, Arc<TransportWorkers>);

/// Transfers a producer's batch slot to a terminal worker when the queue fills.
pub(super) struct BatchPermit {
    gate: BatchGate,
}

impl BatchGate {
    /// Register deferred batch workers with their helper generation.
    pub(super) fn new(workers: Arc<TransportWorkers>) -> Self {
        Self(Arc::default(), workers)
    }

    /// Skip new work once a terminal batch starts, even while its worker waits for bytes.
    pub(super) fn enter_active(&self) -> Result<Option<BatchPermit>> {
        self.acquire()
    }

    /// Queue terminal notices behind final delivery without blocking the stdout reader.
    pub(super) fn enter_or_defer(
        &self,
        bytes: usize,
        later: impl FnOnce(BatchPermit) + Send + 'static,
    ) -> Result<Option<BatchPermit>> {
        let (lock, changed) = &*self.0;
        let mut state = lock
            .lock()
            .map_err(|_| anyhow!("Claude batch gate poisoned"))?;
        while state.busy {
            if state.terminal {
                if bytes > 0
                    && (state.deferred.len() >= 1024
                        || state.deferred_bytes + bytes > super::event_queue::MAX_EVENT_BYTES)
                {
                    return Err(anyhow!(
                        "Claude terminal notices exceeded the bounded queue"
                    ));
                }
                state.deferred_bytes += bytes;
                state.deferred.push_back((bytes, Box::new(later)));
                return Ok(None);
            }
            state = changed
                .wait(state)
                .map_err(|_| anyhow!("Claude batch gate poisoned"))?;
        }
        state.busy = true;
        Ok(Some(BatchPermit { gate: self.clone() }))
    }

    fn acquire(&self) -> Result<Option<BatchPermit>> {
        let (lock, changed) = &*self.0;
        let mut state = lock
            .lock()
            .map_err(|_| anyhow!("Claude batch gate poisoned"))?;
        loop {
            if state.terminal {
                return Ok(None);
            }
            if !state.busy {
                state.busy = true;
                return Ok(Some(BatchPermit { gate: self.clone() }));
            }
            state = changed
                .wait(state)
                .map_err(|_| anyhow!("Claude batch gate poisoned"))?;
        }
    }
}

impl BatchPermit {
    /// Wake waiting producers so only ordered stdout notices follow terminal delivery.
    pub(super) fn mark_terminal(&self) {
        let (lock, changed) = &*self.gate.0;
        if let Ok(mut state) = lock.lock() {
            state.terminal = true;
            changed.notify_all();
        }
    }
}

impl Drop for BatchPermit {
    fn drop(&mut self) {
        let (lock, changed) = &*self.gate.0;
        if let Ok(mut state) = lock.lock() {
            if let Some((bytes, later)) = state.deferred.pop_front() {
                state.deferred_bytes -= bytes;
                let permit = BatchPermit {
                    gate: self.gate.clone(),
                };
                // The next worker owns the same slot; no later producer can overtake it.
                self.gate.1.spawn(move || later(permit));
            } else {
                state.busy = false;
                changed.notify_all();
            }
        }
    }
}
