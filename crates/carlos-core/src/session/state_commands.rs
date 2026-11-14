//! Change thread, status, and context facts through explicit commands.

use super::context_usage::ContextUsage;
use super::state::SessionState;

impl SessionState {
    /// Store backend or turn status text.
    pub fn set_status(&mut self, text: impl Into<String>) {
        self.status = text.into();
    }

    /// Adopt a thread id after the app has reset its backend-specific children.
    pub fn set_thread_id(&mut self, id: String) {
        if self.thread_id != id {
            self.task_stops.clear();
            self.retire_backend_command();
            self.events.current_turn = None;
        }
        self.thread_id = id;
    }

    /// Store an already sanitized name, or clear the old thread's name.
    pub fn set_thread_name(&mut self, name: Option<String>) {
        self.thread_name = name;
    }

    /// Store an accepted native or provisional turn id.
    pub fn set_active_turn_id(&mut self, id: Option<String>) {
        if id.is_none() && self.active_turn_id.is_some() {
            self.messages
                .finish_from(self.turn_suffix_start().unwrap_or(0));
            self.events.turn_started_at = None;
        }
        if self.active_turn_id != id {
            self.events.current_turn = None;
        }
        self.active_turn_id = id;
    }

    /// Store the latest accepted context counts.
    pub fn set_context_usage(&mut self, usage: Option<ContextUsage>) {
        self.context_usage = usage;
    }

    /// Start the current turn's suffix at the end of an adopted history.
    pub(crate) fn restore_turn_boundary(&mut self) {
        if self.active_turn_id.is_some() {
            self.turn_boundary = Some(super::turns::TurnBoundary {
                offset: self.messages.len(),
            });
        } else {
            self.clear_turn_boundary();
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    /// Build the same saved suffix used by the old app test fixtures.
    pub fn set_turn_start_for_test(&mut self, start: Option<usize>) {
        self.turn_boundary = start.map(|offset| super::turns::TurnBoundary { offset });
    }

    #[cfg(any(test, feature = "test-support"))]
    /// Insert a supplied fixture row through the owner.
    pub fn insert_entry_for_test(
        &mut self,
        index: usize,
        message: crate::session::models::Message,
    ) {
        self.messages.insert_for_test(index, message);
    }
}
