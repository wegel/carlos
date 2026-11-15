//! Adopt a complete native snapshot before replaying its charged journal one frame at a time.

use serde_json::json;

use super::frame_batch::NativeBatch;
use crate::backend::BackendClient;
use crate::session::backend_update::{BackendChange, BackendUpdate};
use crate::session::state::SessionState;

impl SessionState {
    /// Adopt the complete native snapshot before starting its ordered journal drain.
    pub(in crate::session) fn begin_native_ready(
        &mut self,
        client: &dyn BackendClient,
        charged: usize,
    ) -> NativeBatch {
        let Some(snapshot) = client.take_reconnect_snapshot(self.codex_connection().generation())
        else {
            return self.failed_native_ready(
                client,
                "Codex reconnect snapshot is unavailable",
                charged,
            );
        };
        let active = self.codex_reconnect_active_turn(&snapshot.thread);
        let goal_changed = self.codex_reconnect_goal_changed();
        let installed =
            active.and_then(|active| super::reconnect_install::install(self, snapshot, active));
        match installed {
            Ok(facts) => {
                let mut batch = NativeBatch::new(charged);
                batch.initial = Some(BackendUpdate::new(BackendChange::Installed(facts)));
                batch.replay = Some(goal_changed);
                batch
            }
            Err(error) => self.failed_native_ready(
                client,
                &format!("Could not rebuild Codex: {error}; draft kept"),
                charged,
            ),
        }
    }

    fn failed_native_ready(
        &mut self,
        client: &dyn BackendClient,
        message: &str,
        charged: usize,
    ) -> NativeBatch {
        self.drain_codex_replay(client);
        let mut batch = NativeBatch::new(charged);
        batch.initial = Some(self.native_frame_closed(&json!({"message":message}), ""));
        batch.abort = true;
        batch
    }
}
