//! Seed native goal fixtures without giving app tests mutable session access.

use std::time::Instant;

use serde_json::Value;

use super::goal_state::GoalKnowledge;
use crate::session::state::SessionState;

impl SessionState {
    /// Set whether the fixture knows if a native goal exists.
    pub fn set_codex_goal_knowledge_for_test(&mut self, knowledge: GoalKnowledge) {
        self.codex_goals.knowledge = knowledge;
    }
    /// Set the native goal document stored by the fixture.
    pub fn set_codex_goal_for_test(&mut self, goal: Value) {
        self.codex_goals.goal = goal;
    }
    /// Set when the fixture last requested its native goal.
    pub fn set_codex_goal_read_time_for_test(&mut self, at: Instant) {
        self.codex_goals.last_read = Some(at);
    }
}
