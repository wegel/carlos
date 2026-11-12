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
}
