//! Expose native goal facts and explicit refresh, close and snapshot commands.

use std::time::Instant;

use super::goal_state::{GoalKnowledge, GoalState};
use crate::session::state::SessionState;

impl SessionState {
    /// Adopt the backend capability to accept direct goal changes.
    pub(crate) fn set_codex_goal_direct_input(&mut self, allowed: Option<bool>) {
        self.codex_goals.direct_input = allowed;
    }
    /// Borrow native goal facts without exposing mutable requests.
    pub(crate) fn codex_goals(&self) -> &GoalState {
        &self.codex_goals
    }
    /// Request a fresh goal read after an explicit user action.
    pub(crate) fn refresh_codex_goal(&mut self) {
        self.codex_goals.last_read = None;
    }
    /// Forget the previous root's goal facts on a thread switch.
    pub(crate) fn reset_codex_goals(&mut self) {
        self.codex_goals = Default::default();
    }
    /// Keep drafts local while fencing pending goal work from a closed connection.
    pub(crate) fn close_codex_goals(&mut self) {
        self.codex_goals.read = None;
        self.codex_goals.mutation = None;
        self.codex_goals.notice.clear();
        self.codex_goals.error = Some("Previous connection closed before the goal change was confirmed. Draft kept; no automatic retry".into());
    }
    /// Seed the validated reconnect goal before applying its original notification path.
    pub(crate) fn begin_codex_goal_snapshot(&mut self, available: bool) {
        self.codex_goals.read = None;
        self.codex_goals.last_read = Some(Instant::now());
        self.codex_goals.knowledge = if available {
            GoalKnowledge::Unknown
        } else {
            GoalKnowledge::Unavailable
        };
    }
    /// Block Ralph after reconnect until the root goal has been read again.
    pub(crate) fn invalidate_codex_goal(&mut self) {
        self.codex_goals.knowledge = GoalKnowledge::Unknown;
        self.codex_goals.last_read = None;
    }
}
