//! Guard Carlos automatic-driver toggles with known native goal ownership.

use anyhow::Result;

use super::item_changes::ItemChanges;
use super::models::Role;
use super::state::SessionState;

impl SessionState {
    /// Keep native goals and a newly requested Carlos loop from driving the same thread.
    pub(crate) fn request_ralph_toggle(&mut self, changes: &mut ItemChanges) -> Result<()> {
        if !self.ralph_enabled() && !self.ralph_runtime().toggle_pending() {
            if let Some(error) = super::codex::goals::ralph_error(self, self.codex_scope()) {
                changes.track(self.append_message(Role::System, error));
                anyhow::bail!(error);
            }
        }
        let status = self.ralph_request_toggle(self.active_turn_id().is_some())?;
        self.set_status(status);
        Ok(())
    }

    /// Recheck native goal state before applying a deferred toggle after a turn.
    pub(crate) fn apply_pending_ralph_toggle(&mut self, changes: &mut ItemChanges) -> Result<()> {
        if self.ralph_runtime().toggle_pending() && !self.ralph_enabled() {
            if let Some(error) = super::codex::goals::ralph_error(self, self.codex_scope()) {
                self.ralph_stop_after_failure();
                changes.track(self.append_message(Role::System, error));
                anyhow::bail!(error);
            }
        }
        if let Some(status) = self.ralph_apply_pending_toggle()? {
            self.set_status(status);
        }
        Ok(())
    }
}
