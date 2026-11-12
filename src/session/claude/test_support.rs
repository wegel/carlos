//! Set up unusual read states previously written directly by app tests.

use crate::session::state::SessionState;

impl SessionState {
    /// Set freshness without creating a backend reply in a test fixture.
    pub(crate) fn set_claude_context_fresh_for_test(&mut self, fresh: bool) {
        self.claude.context.fresh = fresh;
    }

    /// Seed the prior native observation without changing runtime choices.
    pub(crate) fn set_claude_observed_model_for_test(&mut self, model: &str) {
        self.claude.observed_model = Some(model.to_string());
    }

    /// Seed explicit or inherited choices for a switch test.
    pub(crate) fn set_claude_inheritance_for_test(&mut self, model: bool, effort: bool) {
        self.claude.inherits_model = model;
        self.claude.inherits_effort = effort;
    }
    /// Begin a stop without a writer so fixtures can inspect the exact run token.
    pub(crate) fn begin_claude_task_stop_for_test(&mut self, id: &str) -> Option<Option<String>> {
        self.claude.tasks.begin_stop(id)
    }
    /// Check the exact server/request identity in a browser fixture.
    pub(crate) fn has_claude_browser_request_for_test(&self, server: &str, id: &str) -> bool {
        self.claude
            .browser
            .contains_key(&(server.to_string(), id.to_string()))
    }
}

#[cfg(test)]
impl SessionState {
    /// Seed an intentionally inconsistent backend flag in the existing tests.
    pub(crate) fn set_claude_enabled_for_test(&mut self, enabled: bool) {
        self.claude.enabled = enabled;
    }
    /// Seed a helper generation without resetting unrelated fixture facts.
    pub(crate) fn set_claude_generation_for_test(&mut self, generation: u64) {
        self.claude.generation = generation;
    }
    /// Enumerate phase gates without submitting a request in metadata fixtures.
    pub(crate) fn set_claude_phase_for_test(&mut self, phase: super::state::ClaudePhase) {
        self.claude.phase = phase;
    }
    /// Seed the existing init guard for metadata gate tests.
    pub(crate) fn set_claude_awaiting_init_for_test(&mut self, awaiting: bool) {
        self.claude.awaiting_reset_init = awaiting;
    }
}
