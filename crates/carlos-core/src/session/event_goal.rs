//! Export native root goals without routing ids, and compare compact stamps before cloning objectives.

use carlos_proto::goals::{Goal, GoalFacts, GoalKnowledge};
use carlos_proto::ids::{GoalRevision, Sequence};
use carlos_proto::session::Backend;

use super::codex::goal_state::GoalKnowledge as NativeKnowledge;
use super::state::SessionState;
use crate::backend::BackendKind;

impl SessionState {
    pub(super) fn publish_goal(&mut self, changes: &mut Vec<super::events::Change>) {
        let goal = self.goal_fingerprint();
        if goal != self.events.goal_fingerprint {
            self.events.goal_fingerprint = goal;
            changes.push(super::events::Change::Goal);
        }
    }

    pub(super) fn goal_revision(&self) -> GoalRevision {
        self.goal_content_hash()
            .to_hex()
            .to_string()
            .parse()
            .expect("goal digest")
    }

    pub(super) fn goal_fingerprint(&self) -> Option<blake3::Hash> {
        if self.events.backend != Backend::Codex || !self.codex_scope().enabled() {
            return None;
        }
        let state = &self.codex_goals;
        let mut digest = blake3::Hasher::new();
        digest.update(self.goal_content_hash().as_bytes());
        digest.update(&[
            u8::from(state.read.is_some()),
            u8::from(state.mutation.is_some()),
        ]);
        hash_text(&mut digest, state.read_error.as_deref().unwrap_or(""));
        hash_text(&mut digest, &state.notice);
        Some(digest.finalize())
    }

    pub(super) fn export_goal(&self) -> Option<GoalFacts> {
        self.goal_fingerprint()?;
        let state = &self.codex_goals;
        let goal = export_value(&state.goal);
        let knowledge = match state.knowledge {
            NativeKnowledge::Known if !state.goal.is_null() && goal.is_none() => {
                GoalKnowledge::Unknown
            }
            NativeKnowledge::Known => GoalKnowledge::Known,
            NativeKnowledge::Unknown => GoalKnowledge::Unknown,
            NativeKnowledge::Unavailable => GoalKnowledge::Unavailable,
        };
        Some(GoalFacts {
            revision: self.goal_revision(),
            knowledge,
            goal,
            reading: state.read.is_some(),
            pending: state.mutation.is_some(),
            direct_input: state.direct_input,
            read_error: state
                .read_error
                .as_deref()
                .map(crate::protocol_events::bounded_diagnostic),
            notice: crate::protocol_events::bounded_diagnostic(&state.notice),
        })
    }

    fn goal_content_hash(&self) -> blake3::Hash {
        let mut digest = blake3::Hasher::new();
        digest.update(b"carlos.goal.v1");
        digest.update(self.events.owner.as_bytes());
        digest.update(self.session_id().as_str().as_bytes());
        hash_text(&mut digest, self.thread_id());
        let (root, helper) = self.command_connection(BackendKind::Codex);
        digest.update(&root.to_le_bytes());
        digest.update(&helper.to_le_bytes());
        digest.update(&self.codex_goals.revision.to_le_bytes());
        digest.update(&[
            self.codex_goals.knowledge as u8,
            self.codex_goals
                .direct_input
                .map_or(0, |allowed| if allowed { 2 } else { 1 }),
            u8::from(self.codex_scope().closed()),
        ]);
        digest.finalize()
    }
}

fn hash_text(digest: &mut blake3::Hasher, text: &str) {
    digest.update(&(text.len() as u64).to_le_bytes());
    digest.update(text.as_bytes());
}

fn export_value(goal: &serde_json::Value) -> Option<Goal> {
    Some(Goal {
        objective: goal["objective"].as_str()?.into(),
        status: goal["status"].as_str()?.into(),
        token_budget: goal["tokenBudget"].as_u64().map(Sequence::new),
        tokens_used: Sequence::new(goal["tokensUsed"].as_u64()?),
        time_used_seconds: Sequence::new(goal["timeUsedSeconds"].as_u64()?),
        created_at: goal["createdAt"].as_i64()?,
        updated_at: goal["updatedAt"].as_i64()?,
    })
}
