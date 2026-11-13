//! Drain one accepted native envelope without acknowledging a held frame early.

use crate::backend::BackendClient;
use crate::session::backend_update::{BackendChange, BackendContext, BackendUpdate};
use crate::session::state::SessionState;

/// Keep only a synchronous envelope's remaining reports and acknowledgment work.
#[must_use = "Drain native reports to acknowledge each original event charge"]
pub struct NativeBatch {
    pub(in crate::session) initial: Option<BackendUpdate>,
    pub(in crate::session) root_charge: Option<usize>,
    frame_charge: Option<usize>,
    pub(in crate::session) replay: Option<bool>,
    pub(in crate::session) abort: bool,
}

impl NativeBatch {
    /// Retain the current envelope's charge until the synchronous drain finishes.
    pub(in crate::session) fn new(charged: usize) -> Self {
        Self {
            initial: None,
            root_charge: Some(charged),
            frame_charge: None,
            replay: None,
            abort: false,
        }
    }

    /// Advance only after the client applied the previous report and its backend action.
    pub fn next(
        &mut self,
        session: &mut SessionState,
        client: &dyn BackendClient,
        context: &BackendContext<'_>,
    ) -> Option<BackendUpdate> {
        if let Some(initial) = self.initial.take() {
            return Some(initial);
        }
        if let Some(charged) = self.frame_charge.take() {
            client.acknowledge_event(charged);
        }
        if let Some(goal_changed) = self.replay {
            while let Some(frame) = session.take_codex_replay_frame() {
                if matches!(
                    frame.event["method"].as_str(),
                    Some("thread/settings/updated" | "thread/initialized" | "thread/name/updated")
                ) {
                    client.acknowledge_event(frame.charged);
                    continue;
                }
                let update = session.apply_native_frame(&frame.event, context);
                self.frame_charge = Some(frame.charged);
                return Some(update);
            }
            self.replay = None;
            let changes = session.finish_codex_reconnect(goal_changed);
            return Some(BackendUpdate::new(BackendChange::Items(changes)));
        }
        if self.abort {
            self.abort = false;
            client.abort_connection();
        }
        if let Some(charged) = self.root_charge.take() {
            client.acknowledge_event(charged);
        }
        None
    }
}
