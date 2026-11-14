//! Keep reconnect generations, bounded replay charges and bootstrap facts in the session.

use std::path::PathBuf;

use serde_json::Value;

use super::connection_state::{ConnectionState, ReplayFrame};
use crate::backend::BackendClient;
use crate::session::approval_state::ApprovalQueue;
use crate::session::entry_id::EntryId;
use crate::session::item_changes::ItemChanges;
use crate::session::models::Role;
use crate::session::state::SessionState;

impl SessionState {
    /// Borrow connection facts without exposing native journals or readers for mutation.
    pub fn codex_connection(&self) -> &ConnectionState {
        &self.codex_connection
    }

    /// Record the launched transport before any native frame can reach the session.
    pub fn configure_codex_connection(&mut self, generation: u64, socket: Option<PathBuf>) {
        self.codex_connection.generation = generation;
        self.codex_connection.socket = socket;
    }

    /// Fence bootstrap frames until the complete native snapshot has been checked.
    pub fn begin_initial_codex_rebuild(&mut self) {
        self.close_codex();
        self.codex_connection.rebuilding = true;
        self.codex_connection.initial_rebuild = true;
        self.codex_connection.initial_choices = self.take_deferred_codex_settings();
    }

    /// Start one explicit reconnect and retire callbacks only after the transport accepts it.
    pub fn reconnect_codex(&mut self, client: &dyn BackendClient) -> Option<Vec<EntryId>> {
        if !self.codex_scope().closed() || self.codex_connection.rebuilding {
            return None;
        }
        let generation = match client.reconnect(self.thread_id()) {
            Ok(generation) => generation,
            Err(error) => {
                self.set_status(format!("Could not reconnect Codex: {error}"));
                return None;
            }
        };
        self.drain_codex_replay(client);
        self.codex_connection.generation = generation;
        self.codex_connection.rebuilding = true;
        self.codex_connection.initial_rebuild = false;
        self.advance_codex_epoch();
        self.clear_codex_callbacks();
        let mut retired = self.clear_approval_queue(ApprovalQueue::Codex);
        retired.extend(self.retire_active_approval());
        self.ralph_stop_after_failure();
        self.set_status(
            "Reconnecting Codex; draft kept. Messages wait for the complete native snapshot",
        );
        Some(retired)
    }

    /// Hold a charged native frame without acknowledging it before replay.
    pub(crate) fn hold_codex_replay(&mut self, event: Value, charged: usize) -> bool {
        let connection = &mut self.codex_connection;
        if connection.bytes + charged > 12 * 1024 * 1024 || connection.journal.len() >= 2048 {
            return false;
        }
        connection.bytes += charged;
        connection.journal.push_back(ReplayFrame { event, charged });
        true
    }

    /// Retire each queued frame charge exactly once when replay cannot continue.
    pub(crate) fn drain_codex_replay(&mut self, client: &dyn BackendClient) {
        for frame in self.codex_connection.journal.drain(..) {
            client.acknowledge_event(frame.charged);
        }
        self.codex_connection.bytes = 0;
    }

    /// Take the next held native frame in its accepted transport order.
    pub(crate) fn take_codex_replay_frame(&mut self) -> Option<ReplayFrame> {
        self.codex_connection.journal.pop_front()
    }

    /// Resolve the live root turn from native snapshot facts and accepted replay frames.
    pub(crate) fn codex_reconnect_active_turn(
        &self,
        thread: &Value,
    ) -> anyhow::Result<Option<String>> {
        super::reconnect_replay::active_turn(thread, &self.codex_connection.journal)
    }

    /// Check only current-root goal events before installing the snapshot.
    pub(crate) fn codex_reconnect_goal_changed(&self) -> bool {
        self.codex_connection.journal.iter().any(|frame| {
            frame.event["params"]["threadId"] == *self.thread_id()
                && matches!(
                    frame.event["method"].as_str(),
                    Some("thread/goal/updated" | "thread/goal/cleared")
                )
        })
    }

    /// Publish reconnect status only after the held journal has drained.
    pub(crate) fn finish_codex_reconnect(&mut self, goal_changed: bool) -> ItemChanges {
        self.codex_connection.bytes = 0;
        self.codex_connection.rebuilding = false;
        self.events.launch = None;
        self.codex_connection.runtime_refresh = true;
        if let Some(choices) = self.codex_connection.initial_choices.take() {
            self.defer_codex_settings(Some(choices));
        }
        if goal_changed {
            self.invalidate_codex_goal();
        }
        let changes = ItemChanges::default();
        if !self.codex_connection.initial_rebuild {
            self.append_message(
                Role::System,
                "Codex reconnected; native work restored. Unconfirmed input was not resent.",
            );
        }
        if self.codex_connection.unreplayed_children > 0 {
            self.append_message(Role::System, "Codex reattached this thread. The server replays this thread’s requests; unanswered child requests may need the client that owns that child. F2 shows its state.");
            self.codex_connection.unreplayed_children = 0;
        }
        self.set_status(
            "Codex reconnected; native work restored. Unconfirmed input was not resent",
        );
        changes
    }
}

impl SessionState {
    /// Stop holding frames before processing a native close or replay overflow.
    pub(crate) fn stop_codex_rebuild(&mut self) {
        self.codex_connection.rebuilding = false;
    }
}
