//! Bounds Claude events until the UI actually handles them, including intermediate queues.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};

use anyhow::{bail, Result};

const MAX_EVENT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone)]
/// Keeps Claude event bytes charged until the UI handles them.
pub(super) struct EventSender {
    pub(super) sender: mpsc::SyncSender<String>,
    bytes: Arc<AtomicUsize>,
    pub(super) generation: u64,
}

impl EventSender {
    /// Create a byte-budgeted event sender.
    pub(super) fn new(sender: mpsc::SyncSender<String>) -> Self {
        Self {
            sender,
            bytes: Arc::new(AtomicUsize::new(0)),
            generation: 1,
        }
    }

    /// Queue one event without exceeding the byte or frame budget.
    pub(super) fn send(&self, line: String) -> Result<()> {
        let line = self.stamp(line);
        let length = line.len();
        if self
            .bytes
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |bytes| {
                bytes
                    .checked_add(length)
                    .filter(|sum| *sum <= MAX_EVENT_BYTES)
            })
            .is_err()
        {
            bail!("Claude UI event backlog exceeded 8 MiB");
        }
        if self.sender.try_send(line).is_err() {
            self.handled(length);
            bail!("Claude UI event queue is full or closed");
        }
        Ok(())
    }

    /// Share the same event budget with a new helper generation.
    pub(super) fn for_generation(&self, generation: u64) -> Self {
        Self {
            sender: self.sender.clone(),
            bytes: self.bytes.clone(),
            generation,
        }
    }

    /// Mark a Claude event so a replaced helper cannot mutate the current session.
    pub(super) fn stamp(&self, line: String) -> String {
        let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&line) else {
            return line;
        };
        value["params"]["generation"] = serde_json::json!(self.generation);
        value.to_string()
    }

    /// Release the bytes of an event the UI has handled.
    pub(super) fn handled(&self, length: usize) {
        let _ = self
            .bytes
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |bytes| {
                Some(bytes.saturating_sub(length))
            });
    }
}
