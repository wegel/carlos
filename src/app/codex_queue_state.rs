//! Keep queue focus and unsent text in the terminal that owns the editors.

use ratatui_textarea::TextArea;
use serde_json::Value;

#[derive(Default)]
pub(super) struct QueueView {
    pub(super) mode: bool,
    pub(super) show: bool,
    pub(super) selected: Option<String>,
    pub(super) edit: Option<QueueEdit>,
    pub(super) recovery: Option<String>,
    pub(super) error: Option<String>,
}

/// Compare the original persisted input before saving a separate textarea draft.
pub(super) struct QueueEdit {
    pub(super) id: String,
    pub(super) original: Value,
    pub(super) editor: TextArea<'static>,
}

impl QueueView {
    /// Keep selection fixed when external clients consume or reorder queued entries.
    pub(super) fn selected_entry<'a>(
        &self,
        facts: &'a crate::session::codex::queue_state::QueueState,
    ) -> Option<&'a Value> {
        let id = self.selected.as_deref()?;
        facts.entries().iter().find(|entry| entry["id"] == id)
    }

    /// Choose a visible entry only after an explicit keyboard navigation.
    pub(super) fn move_selection(
        &mut self,
        direction: isize,
        facts: &crate::session::codex::queue_state::QueueState,
    ) {
        if facts.entries().is_empty() {
            self.selected = None;
            return;
        }
        let current = facts
            .entries()
            .iter()
            .position(|entry| entry["id"].as_str() == self.selected.as_deref());
        let next = current.map_or(0, |index| {
            (index as isize + direction).rem_euclid(facts.entries().len() as isize) as usize
        });
        self.selected = facts.entries()[next]["id"].as_str().map(str::to_string);
    }
}
