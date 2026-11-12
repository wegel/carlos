//! Seed native goal fixtures without giving app tests mutable session access.

use std::time::Instant;

use serde_json::Value;

use super::goal_state::GoalKnowledge;
use crate::session::state::SessionState;

impl SessionState {
    pub(crate) fn set_codex_goal_knowledge_for_test(&mut self, knowledge: GoalKnowledge) {
        self.codex_goals.knowledge = knowledge;
    }
    pub(crate) fn set_codex_goal_for_test(&mut self, goal: Value) {
        self.codex_goals.goal = goal;
    }
    pub(crate) fn set_codex_goal_read_time_for_test(&mut self, at: Instant) {
        self.codex_goals.last_read = Some(at);
    }
}
