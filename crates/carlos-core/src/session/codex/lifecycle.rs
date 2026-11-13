//! Fence terminal Codex failures and return explicit client editor recovery effects.

use serde_json::{Map, Value};

use super::settings::{self, SettingsUpdate};
use crate::session::approval_state::ApprovalQueue;
use crate::session::entry_id::EntryId;
use crate::session::item_changes::ItemChanges;
use crate::session::models::{Role, TurnStatus};
use crate::session::state::SessionState;
use crate::session::text::safe_text;

/// Tell a caller which local controls to retire after one native transport close.
pub struct CodexClosed {
    pub settings: SettingsUpdate,
    pub recovery: Option<String>,
    pub retired: Vec<EntryId>,
    pub transcript: ItemChanges,
    pub hint: String,
}

impl SessionState {
    /// Close a native generation once, retaining accepted text for explicit client recovery.
    pub(crate) fn close_codex_native(
        &mut self,
        params: &Map<String, Value>,
        guidance: &str,
    ) -> Option<CodexClosed> {
        if self.codex_connection.closed_generation == Some(self.codex_connection.generation) {
            return None;
        }
        self.codex_connection.closed_generation = Some(self.codex_connection.generation);
        self.codex_connection.rebuilding = false;
        self.close_codex();
        self.clear_rate_limits();
        self.codex_connection.streams = Default::default();
        self.codex_connection.runtime_read = None;
        self.codex_connection.runtime_refresh = false;
        if let Some(turn) = self.active_turn_id().clone() {
            if self.codex_connection.socket.is_none() {
                self.record_turn_completed(&turn, TurnStatus::Failed);
            } else {
                self.set_active_turn_id(None);
            }
        }
        self.ralph_stop_after_failure();
        let mut settings = SettingsUpdate::default();
        settings::closed(self, &mut settings);
        let recovery = self.take_codex_queue_recovery();
        let retired = self.close_codex_controls();
        let message = safe_text(
            params
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("Codex closed"),
        );
        let label = if self.codex_connection.socket.is_some() {
            "Codex connection lost"
        } else {
            "Codex stopped"
        };
        let mut transcript = ItemChanges::default();
        transcript.track(self.append_message(Role::System, format!("{label}: {message}{guidance}. F5 reconnects; drafts are kept. Unconfirmed input will not be resent.")));
        self.set_status(format!("{label}: {message}"));
        Some(CodexClosed {
            settings,
            recovery,
            retired,
            transcript,
            hint: format!("{label}: {message}{guidance}. F5 reconnects"),
        })
    }

    fn close_codex_controls(&mut self) -> Vec<EntryId> {
        self.close_codex_goals();
        self.close_codex_panel();
        self.close_codex_queue();
        if self.codex_connection.socket.is_some() {
            let children = self
                .codex_callbacks()
                .iter()
                .map(|(_, entry)| entry)
                .filter(|entry| entry.params()["threadId"] != *self.thread_id())
                .count();
            self.codex_connection.unreplayed_children =
                self.codex_connection.unreplayed_children.max(children);
        }
        self.clear_codex_callbacks();
        let mut retired = self.clear_approval_queue(ApprovalQueue::Codex);
        retired.extend(self.retire_active_approval());
        self.clear_turn_boundary();
        retired
    }
}
