//! Pass tool observations to the session and redraw only the labels it changed.

use std::sync::mpsc;

use carlos_core::summaries::SummaryEvent;

use super::state::AppState;

impl AppState {
    pub(super) fn configure_summary_worker(&mut self, tx: mpsc::Sender<SummaryEvent>) {
        self.session.configure_summary_worker(tx);
    }

    pub(super) fn handle_summary_event(&mut self, event: SummaryEvent) {
        self.session.handle_summary_event(event);
        self.consume_core_events();
    }

    pub(super) fn apply_summary_setting(&mut self) {
        self.session
            .apply_summary_setting(self.view.summary_enabled_draft);
        self.consume_core_events();
    }
}

#[cfg(test)]
impl AppState {
    pub(super) fn assert_entry_invariants(&self) {
        self.assert_approval_hint_invariants();
        self.session.messages().assert_invariants();
        self.session
            .input_history()
            .assert_invariants(self.session.messages());
        self.session.assert_summary_invariants();
        for prompt in self.session.claude_history().unknown() {
            assert!(self.session.messages().entry(prompt.entry_id()).is_some());
        }
    }
}
