//! Change thread, status, and context facts through explicit commands.

use super::context_usage::ContextUsage;
use super::state::SessionState;

impl SessionState {
    /// Store backend or turn status text.
    pub(crate) fn set_status(&mut self, text: impl Into<String>) {
        self.status = text.into();
    }

    /// Adopt a thread id after the app has reset its backend-specific children.
    pub(crate) fn set_thread_id(&mut self, id: String) {
        self.thread_id = id;
    }

    /// Store an already sanitized name, or clear the old thread's name.
    pub(crate) fn set_thread_name(&mut self, name: Option<String>) {
        self.thread_name = name;
    }

    /// Store an accepted native or provisional turn id.
    pub(crate) fn set_active_turn_id(&mut self, id: Option<String>) {
        self.active_turn_id = id;
    }

    /// Store the latest accepted context counts.
    pub(crate) fn set_context_usage(&mut self, usage: Option<ContextUsage>) {
        self.context_usage = usage;
    }

    /// Start the current turn's suffix at the end of an adopted history.
    pub(crate) fn restore_turn_boundary(&mut self) {
        if self.active_turn_id.is_some() {
            self.mark_turn_started();
        } else {
            self.clear_turn_boundary();
        }
    }

    #[cfg(test)]
    /// Build the same saved suffix used by the old app test fixtures.
    pub(crate) fn set_turn_start_for_test(&mut self, start: Option<usize>) {
        self.turn_boundary = start.map(|offset| super::turns::TurnBoundary { offset });
    }

    #[cfg(test)]
    /// Insert a supplied fixture row through the owner.
    pub(crate) fn insert_entry_for_test(&mut self, index: usize, message: super::models::Message) {
        self.messages.insert_for_test(index, message);
    }
}
