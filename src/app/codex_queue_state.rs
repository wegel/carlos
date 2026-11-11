//! Keep native queue snapshots and editor drafts separate from consumed conversation items.

use std::collections::HashSet;
use std::time::Instant;

use ratatui_textarea::TextArea;
use serde_json::Value;

#[derive(Default)]
/// Keep a bounded native snapshot and its independent control/editor state.
pub(super) struct QueueState {
    pub(super) mode: bool,
    pub(super) show: bool,
    pub(super) known: bool,
    pub(super) revision: u64,
    pub(super) entries: Vec<Value>,
    pub(super) selected: Option<String>,
    pub(super) read: Option<QueueRead>,
    pub(super) operation: Option<QueueOperation>,
    pub(super) edit: Option<QueueEdit>,
    pub(super) recovery: Option<String>,
    pub(super) notice: String,
    pub(super) error: Option<String>,
}

/// Assemble one snapshot before replacing the last complete native view.
pub(super) struct QueueRead {
    pub(super) token: u64,
    pub(super) revision: u64,
    pub(super) started: Instant,
    pub(super) entries: Vec<Value>,
    pub(super) cursors: HashSet<String>,
    pub(super) bytes: usize,
}

/// Keep one exact mutation identity and any locally submitted text until acknowledgement.
pub(super) struct QueueOperation {
    pub(super) token: u64,
    pub(super) method: String,
    pub(super) params: Value,
    pub(super) submitted: Option<String>,
}

/// Compare the original persisted input before saving a separate textarea draft.
pub(super) struct QueueEdit {
    pub(super) id: String,
    pub(super) original: Value,
    pub(super) editor: TextArea<'static>,
}

impl QueueState {
    /// Invalidate the snapshot without replacing the selected native identity or draft.
    pub(super) fn invalidate(&mut self) {
        self.revision = self.revision.saturating_add(1);
        self.known = false;
    }

    /// Keep selection fixed when external clients consume or reorder queued entries.
    pub(super) fn selected_entry(&self) -> Option<&Value> {
        let id = self.selected.as_deref()?;
        self.entries.iter().find(|entry| entry["id"] == id)
    }

    /// Choose a visible entry only after an explicit keyboard navigation.
    pub(super) fn move_selection(&mut self, direction: isize) {
        if self.entries.is_empty() {
            self.selected = None;
            return;
        }
        let current = self
            .entries
            .iter()
            .position(|entry| entry["id"].as_str() == self.selected.as_deref());
        let next = current.map_or(0, |index| {
            (index as isize + direction).rem_euclid(self.entries.len() as isize) as usize
        });
        self.selected = self.entries[next]["id"].as_str().map(str::to_string);
    }
}

/// Edit only the plain text shape that Carlos can reproduce without losing attachments/spans.
pub(super) fn editable_text(entry: &Value) -> Option<&str> {
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
