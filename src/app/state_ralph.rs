//! Guard Carlos automatic-driver toggles with known native goal ownership.

use anyhow::Result;

use super::state::AppState;

impl AppState {
    /// Keep native goals and a newly requested Carlos loop from driving the same thread.
    pub(super) fn request_ralph_toggle(&mut self) -> Result<()> {
        if !self.ralph_enabled() && !self.ralph_runtime.toggle_pending() {
            if let Some(error) = super::codex_goals::ralph_error(self) {
                self.append_message(super::Role::System, error);
                anyhow::bail!(error);
            }
        }
        let status = self
            .ralph_runtime
            .request_toggle(self.active_turn_id.is_some())?;
        self.set_status(status);
        Ok(())
    }

    /// Recheck native goal state before applying a deferred toggle after a turn.
    pub(super) fn apply_pending_ralph_toggle(&mut self) -> Result<()> {
        if self.ralph_runtime.toggle_pending() && !self.ralph_enabled() {
            if let Some(error) = super::codex_goals::ralph_error(self) {
                self.ralph_runtime.stop_after_failure();
                self.append_message(super::Role::System, error);
                anyhow::bail!(error);
            }
        }
        if let Some(status) = self.ralph_runtime.apply_pending_toggle()? {
            self.set_status(status);
        }
        Ok(())
    }
}
