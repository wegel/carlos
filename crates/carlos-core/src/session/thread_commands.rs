//! Adopt native thread identities and report the client controls that must reset.

use super::codex::callback_commands::CallbackRetired;
use super::state::SessionState;

/// Report retired callback tokens and the exact family of local controls to reset.
#[derive(Default)]
pub struct ThreadChanged {
    pub codex_reset: bool,
    pub callbacks: Vec<CallbackRetired>,
}

impl SessionState {
    /// Adopt a checked native id while preserving the existing independent backend flags.
    pub fn adopt_native_thread(&mut self, thread_id: String) -> ThreadChanged {
        let mut changed = ThreadChanged::default();
        if *self.thread_id() != thread_id {
            if self.codex_scope().enabled() {
                for id in self.codex_callback_ids() {
                    changed.callbacks.push(self.retire_codex_callback(&id));
                }
                self.reset_codex_panel();
                self.reset_codex_task_status();
                self.reset_codex_goals();
                self.advance_codex_epoch();
                self.reset_codex_queue();
                self.reset_codex_settings();
                changed.codex_reset = true;
            }
            self.set_thread_name(None);
        }
        self.set_thread_id(thread_id);
        changed
    }
}
