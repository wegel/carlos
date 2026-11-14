//! Own native queue snapshots and submitted writes without client editors.

use std::collections::HashSet;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Instant;

use serde_json::Value;

#[derive(Debug, Default)]
/// Own accepted native queued prompts and their pending reads and changes.
pub struct QueueState {
    pub(in crate::session) exhausted: bool,
    pub(in crate::session) read_error: Option<String>,
    pub(in crate::session) known: bool,
    pub(in crate::session) revision: u64,
    pub(in crate::session) entries: Vec<Value>,
    pub(in crate::session) read: Option<QueueRead>,
    pub(in crate::session) operation: Option<QueueOperation>,
    pub(in crate::session) notice: String,
    pub(in crate::session) error: Option<String>,
}
/// Assemble one snapshot before replacing the last complete native view.
#[derive(Debug)]
pub(crate) struct QueueRead {
    pub(in crate::session) token: u64,
    pub(in crate::session) revision: u64,
    pub(in crate::session) started: Instant,
    pub(in crate::session) entries: Vec<Value>,
    pub(in crate::session) cursors: HashSet<String>,
    pub(in crate::session) bytes: usize,
}

/// Keep one exact mutation identity and any locally submitted text until acknowledgement.
#[derive(Debug)]
pub(crate) struct QueueOperation {
    pub(in crate::session) live: Arc<AtomicBool>,
    pub(in crate::session) operation: Option<carlos_proto::submissions::ClientOperation>,
    pub(in crate::session) token: u64,
    pub(in crate::session) method: String,
    pub(in crate::session) params: Value,
    pub(in crate::session) submitted: Option<String>,
}

impl Drop for QueueOperation {
    fn drop(&mut self) {
        self.live.store(false, Ordering::SeqCst);
    }
}

impl QueueState {
    /// Borrow native entries from the last complete read.
    pub fn entries(&self) -> &[Value] {
        &self.entries
    }
    /// Read whether the complete queue is still authoritative.
    pub fn known(&self) -> bool {
        self.known
    }
    /// Read whether a bounded snapshot is being assembled.
    pub fn reading(&self) -> bool {
        self.read.is_some()
    }
    /// Read whether a native queue write remains uncertain.
    pub fn pending(&self) -> bool {
        self.operation.is_some()
    }
    /// Borrow the most recent native queue failure.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
    /// Borrow the last native queue acknowledgment.
    pub fn notice(&self) -> &str {
        &self.notice
    }
    pub(in crate::session) fn invalidate(&mut self) {
        match self.revision.checked_add(1) {
            Some(revision) => self.revision = revision,
            None => {
                self.exhausted = true;
                self.read_error =
                    Some("Queue revision exhausted; reconnect before changing it".into());
            }
        }
        self.known = false;
    }
}
/// Edit only the plain text shape that Carlos can reproduce without losing attachments/spans.
pub fn editable_text(entry: &Value) -> Option<&str> {
    let input = entry["input"].as_array()?;
    let [text] = input.as_slice() else {
        return None;
    };
    let object = text.as_object()?;
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "type" | "text" | "text_elements"))
        || text["type"] != "text"
        || text
            .get("text_elements")
            .is_some_and(|spans| !spans.as_array().is_some_and(Vec::is_empty))
    {
        return None;
    }
    text["text"].as_str()
}
