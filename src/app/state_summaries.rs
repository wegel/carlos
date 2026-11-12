//! Pass tool observations to the session and redraw only the labels it changed.

use std::sync::mpsc;

use serde_json::Value;

use super::state::AppState;
use crate::session::entry_id::EntryId;
use crate::summaries::SummaryEvent;

impl AppState {
    pub(super) fn with_historical_tools<T>(&mut self, read: impl FnOnce(&mut Self) -> T) -> T {
        let previous = self.session.set_summary_replaying(true);
        let result = read(self);
        self.session.set_summary_replaying(previous);
        result
    }

    pub(super) fn observe_summary_item(&mut self, item: &Value, live: bool) {
        let Some(id) = item["id"].as_str() else {
            return;
        };
        let scope = self
            .codex
            .item_turn
            .as_deref()
            .filter(|_| self.codex.enabled)
            .map(|turn| (turn, self.codex.item_part));
        let entry = self.session.summary_cache_entry(id, scope);
        let update = self
            .session
            .observe_summary_item(item, live, &self.item_key(id), entry);
        self.apply_session_update(update);
    }

    pub(super) fn configure_summary_worker(&mut self, tx: mpsc::Sender<SummaryEvent>) {
        self.session.configure_summary_worker(tx);
    }

    pub(super) fn handle_summary_event(&mut self, event: SummaryEvent) {
        let changed = self.session.handle_summary_event(event);
        self.redraw_summary_entries(changed);
    }

    pub(super) fn apply_summary_setting(&mut self) {
        let changed = self
            .session
            .apply_summary_setting(self.view.summary_enabled_draft);
        self.redraw_summary_entries(changed);
    }

    fn redraw_summary_entries(&mut self, changed: Vec<EntryId>) {
        for id in changed {
            if let Some(index) = self.session.messages().position(id) {
                self.mark_transcript_dirty_from(index);
            }
        }
    }
}

#[cfg(test)]
impl AppState {
    pub(super) fn summary_binding_for_test(&self, item: &str) -> Option<String> {
        let id = self
            .session
            .messages()
            .entry_id_for_item(&self.item_key(item))?;
        self.session.summary_binding_for_test(id)
    }

    pub(super) fn summary_token_for_test(&self, item: &str) -> String {
        self.summary_binding_for_test(item)
            .expect("summary binding")
    }
}

#[cfg(test)]
impl AppState {
    pub(super) fn assert_entry_invariants(&self) {
        self.assert_approval_hint_invariants();
        self.session.messages().assert_invariants();
        self.session
            .input_history()
            .assert_invariants(&self.session.messages());
        self.session.assert_summary_invariants();
        for prompt in &self.claude.history.unknown {
            assert!(self.session.messages().entry(prompt.entry_id).is_some());
        }
    }
}
