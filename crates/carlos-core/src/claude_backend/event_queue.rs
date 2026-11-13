//! Bounds Claude events until the consumer handles them, including intermediate queues.

use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::time::Duration;

use anyhow::{anyhow, bail, Result};

use super::event_batch::BatchPermit;
use super::transport_workers::TransportWorkers;
use crate::backend::EventReceiver;

pub(super) const MAX_EVENT_BYTES: usize = 8 * 1024 * 1024;

/// Keeps delivered bytes charged until the consumer acknowledges their actual length.
#[derive(Clone)]
pub(super) struct EventSender {
    sender: mpsc::SyncSender<String>,
    budget: Arc<(Mutex<Budget>, Condvar)>,
    pub(super) workers: Arc<TransportWorkers>,
    #[cfg(any(test, feature = "test-support"))]
    pub(super) hooks: Arc<super::transport_test_hooks::TestHooks>,
    pub(super) generation: u64,
}

#[derive(Default)]
struct Budget {
    bytes: usize,
    closed: bool,
}

impl EventSender {
    /// Create a byte-budgeted event sender.
    fn new(sender: mpsc::SyncSender<String>) -> Self {
        Self {
            sender,
            budget: Arc::new((Mutex::new(Budget::default()), Condvar::new())),
            workers: Arc::default(),
            #[cfg(any(test, feature = "test-support"))]
            hooks: Arc::default(),
            generation: 1,
        }
    }

    /// Keep teardown cancellation local to one helper, even when generations share a queue.
    pub(super) fn with_workers(mut self, workers: Arc<TransportWorkers>) -> Self {
        self.workers = workers;
        self
    }

    /// Create a queue whose receiver drop wakes every blocked terminal sender.
    pub(super) fn channel(capacity: usize) -> (Self, EventReceiver) {
        let (sender, receiver) = mpsc::sync_channel(capacity);
        let events = Self::new(sender);
        let budget = events.budget.clone();
        let receiver = EventReceiver::with_close(receiver, move || {
            if let Ok(mut state) = budget.0.lock() {
                state.closed = true;
                budget.1.notify_all();
            }
        });
        (events, receiver)
    }

    /// Mark a reader boundary for deterministic tests; production has no callback.
    pub(super) fn checkpoint(&self, name: &'static str) {
        #[cfg(any(test, feature = "test-support"))]
        self.hooks.reach(name);
        #[cfg(not(test))]
        let _ = name;
    }

    /// Queue one event without exceeding the byte or frame budget.
    pub(super) fn send(&self, line: String) -> Result<()> {
        let line = self.stamp(line);
        let length = line.len();
        self.charge(length, false)?;
        if self.sender.try_send(line).is_err() {
            self.handled(length);
            bail!("Claude UI event queue is full or closed");
        }
        Ok(())
    }

    /// Deliver final failure facts in order after stopping the helper, without blocking shutdown.
    pub(super) fn finish(&self, lines: Vec<String>, batch: BatchPermit, stop: impl FnOnce()) {
        let mut stop = Some(stop);
        self.checkpoint("finish");
        let mut lines = lines.into_iter();
        while let Some(line) = lines.next() {
            if self.send(line.clone()).is_ok() {
                continue;
            }
            if let Some(stop) = stop.take() {
                stop();
            }
            self.checkpoint("terminal_deferred");
            let events = self.clone();
            self.workers.spawn(move || {
                for line in std::iter::once(line).chain(lines) {
                    let line = events.stamp(line);
                    let length = line.len();
                    if events.charge(length, true).is_err() {
                        break;
                    }
                    if events.send_terminal(line).is_err() {
                        events.handled(length);
                        break;
                    }
                }
                drop(batch);
                events.checkpoint("finished");
            });
            break;
        }
    }

    /// Share the same event budget with a new helper generation.
    pub(super) fn for_generation(&self, generation: u64) -> Self {
        Self {
            sender: self.sender.clone(),
            budget: self.budget.clone(),
            workers: self.workers.clone(),
            #[cfg(any(test, feature = "test-support"))]
            hooks: self.hooks.clone(),
            generation,
        }
    }

    fn send_terminal(&self, mut line: String) -> Result<()> {
        loop {
            if self.workers.is_cancelled() {
                bail!("Claude helper generation is closed");
            }
            match self.sender.try_send(line) {
                Ok(()) => return Ok(()),
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    bail!("Claude UI event receiver is closed");
                }
                Err(mpsc::TrySendError::Full(returned)) => line = returned,
            }
            // The forwarder can free a frame slot before the UI acknowledges its bytes.
            let budget = self.budget.0.lock().expect("event budget");
            let _ = self
                .budget
                .1
                .wait_timeout(budget, Duration::from_millis(10));
        }
    }

    /// Fence every message from a replaced helper, including common protocol messages.
    pub(super) fn stamp(&self, line: String) -> String {
        let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&line) else {
            return line;
        };
        value["params"]["generation"] = serde_json::json!(self.generation);
        value.to_string()
    }

    /// Release exactly the bytes of one delivered event.
    pub(super) fn handled(&self, length: usize) {
        if let Ok(mut bytes) = self.budget.0.lock() {
            bytes.bytes = bytes.bytes.saturating_sub(length);
            self.budget.1.notify_all();
        }
    }

    /// Inspect outstanding delivery charges in receiver-level tests.
    #[cfg(any(test, feature = "test-support"))]
    pub(super) fn charged_bytes(&self) -> usize {
        self.budget.0.lock().expect("event budget").bytes
    }

    fn charge(&self, length: usize, wait: bool) -> Result<()> {
        if length > MAX_EVENT_BYTES {
            bail!("Claude UI event backlog exceeded 8 MiB");
        }
        let mut bytes = self
            .budget
            .0
            .lock()
            .map_err(|_| anyhow!("Claude event budget lock poisoned"))?;
        while !bytes.closed
            && !self.workers.is_cancelled()
            && bytes.bytes + length > MAX_EVENT_BYTES
        {
            if !wait {
                bail!("Claude UI event backlog exceeded 8 MiB");
            }
            self.checkpoint("budget_wait");
            bytes = self
                .budget
                .1
                .wait_timeout(bytes, Duration::from_millis(10))
                .map_err(|_| anyhow!("Claude event budget lock poisoned"))?
                .0;
        }
        if bytes.closed || self.workers.is_cancelled() {
            bail!("Claude UI event receiver is closed");
        }
        bytes.bytes += length;
        Ok(())
    }
}
