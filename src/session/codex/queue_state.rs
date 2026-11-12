//! Own native queue snapshots and submitted writes without client editors.

use std::collections::HashSet;
use std::time::Instant;

use serde_json::Value;

#[derive(Debug, Default)]
pub(crate) struct QueueState {
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
    pub(in crate::session) token: u64,
    pub(in crate::session) method: String,
    pub(in crate::session) params: Value,
    pub(in crate::session) submitted: Option<String>,
}

impl QueueState {
    /// Borrow native entries from the last complete read.
    pub(crate) fn entries(&self) -> &[Value] {
        &self.entries
    }
    /// Read whether the complete queue is still authoritative.
    pub(crate) fn known(&self) -> bool {
        self.known
    }
    /// Read whether a bounded snapshot is being assembled.
    pub(crate) fn reading(&self) -> bool {
        self.read.is_some()
    }
    /// Read whether a native queue write remains uncertain.
    pub(crate) fn pending(&self) -> bool {
        self.operation.is_some()
    }
    /// Borrow the most recent native queue failure.
    pub(crate) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
    /// Borrow the last native queue acknowledgment.
    pub(crate) fn notice(&self) -> &str {
        &self.notice
    }
    pub(in crate::session) fn invalidate(&mut self) {
        self.revision = self.revision.saturating_add(1);
        self.known = false;
    }
}
/// Edit only the plain text shape that Carlos can reproduce without losing attachments/spans.
pub(crate) fn editable_text(entry: &Value) -> Option<&str> {
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
