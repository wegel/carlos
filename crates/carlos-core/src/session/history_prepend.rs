//! Restore facts for older entries without replacing newer rows or publishing a transcript reset.

use serde_json::Value;

use super::history_receive::RestoreMode;
use super::item_history_draft::build;
use super::state::SessionState;

impl SessionState {
    pub(super) fn prepend_native_thread(&mut self, thread: &Value) -> usize {
        let previous = self.replace_codex_item_turn(None);
        let draft = build(self, thread);
        let mut journal = std::mem::take(&mut self.messages.journal);
        let ids = self.messages.prepend_history(draft.entries);
        for (turn, status) in draft.turns {
            self.record_historical_turn_status(&turn, status);
        }
        let mut prompts = Vec::new();
        for record in draft.records.iter().rev() {
            super::codex::panel_events::report_older(self, &record.item);
        }
        for record in draft.records {
            for (index, text) in &record.prompts {
                prompts.push((ids[*index], text.clone()));
            }
            self.restore_history_record(record, RestoreMode::Prepend);
        }
        self.input_history.prepend_history(prompts);
        if let Some(boundary) = &mut self.turn_boundary {
            boundary.offset += ids.len();
        }
        journal.prepend_stored(ids.len());
        self.messages.journal = journal;
        self.replace_codex_item_turn(previous);
        ids.len()
    }
}
