//! Apply explicit Ralph commands without exposing mutable runtime facts.

use anyhow::Result;
use carlos_proto::submissions::SubmittedText;

use super::state::SessionState;
use crate::backend::PromptOrigin;

impl SessionState {
    /// Stop automatic work and keep queued human prompts.
    pub(crate) fn ralph_stop_after_failure(&mut self) {
        self.ralph_runtime.stop_after_failure()
    }

    /// Return queued human text once after a backend failure.
    pub(crate) fn ralph_take_queued_human_drafts(&mut self) -> Vec<SubmittedText> {
        self.ralph_runtime.take_queued_human_drafts()
    }

    /// Queue accepted human text with its recovery owner.
    pub fn queue_submitted_input(&mut self, draft: SubmittedText, record: bool) {
        self.ralph_runtime
            .enqueue_submitted(draft, record, PromptOrigin::Human);
    }

    pub(crate) fn enqueue_submitted_prompt(
        &mut self,
        draft: SubmittedText,
        record: bool,
        origin: PromptOrigin,
    ) {
        self.ralph_runtime.enqueue_submitted(draft, record, origin);
    }

    /// Toggle immediately or defer the change while a turn runs.
    pub fn ralph_request_toggle(&mut self, turn_active: bool) -> Result<&'static str> {
        self.ralph_runtime.request_toggle(turn_active)
    }

    /// Apply one deferred toggle after the app checks native goal ownership.
    pub(crate) fn ralph_apply_pending_toggle(&mut self) -> Result<Option<&'static str>> {
        self.ralph_runtime.apply_pending_toggle()
    }

    /// Clear the waiting-for-input flag after a human submits a prompt.
    pub fn ralph_mark_user_turn_submitted(&mut self) {
        self.ralph_runtime.mark_user_turn_submitted()
    }

    /// Keep accepted prompt text together with its human or automatic source.
    pub fn ralph_enqueue_prompt(
        &mut self,
        text: String,
        record_input_history: bool,
        origin: PromptOrigin,
    ) {
        self.ralph_runtime
            .enqueue_prompt(text, record_input_history, origin)
    }
}
