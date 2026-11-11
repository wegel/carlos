//! Keep native root-goal facts and an independent editor without creating chat prompts.

use std::time::Instant;

use ratatui_textarea::TextArea;
use serde_json::Value;

/// Only known non-driving state may permit Carlos to enable a separate automatic driver.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum GoalKnowledge {
    #[default]
    Unknown,
    Known,
    Unavailable,
}

/// Retain native facts, bounded pending work and explicit user drafts for one root epoch.
#[derive(Default)]
pub(super) struct GoalState {
    pub(super) show: bool,
    pub(super) knowledge: GoalKnowledge,
    pub(super) goal: Value,
    pub(super) revision: u64,
    pub(super) read: Option<GoalRequest>,
    pub(super) mutation: Option<GoalRequest>,
    pub(super) editor: Option<GoalDraft>,
    pub(super) editing: bool,
    pub(super) confirm: Option<GoalConfirm>,
    pub(super) error: Option<String>,
    pub(super) read_error: Option<String>,
    pub(super) notice: String,
    pub(super) last_read: Option<Instant>,
    pub(super) direct_input: Option<bool>,
    pub(super) scroll: usize,
}

/// Match each acknowledgement to its exact scope and the facts known before dispatch.
pub(super) struct GoalRequest {
    pub(super) token: u64,
    pub(super) method: String,
    pub(super) params: Value,
    pub(super) revision: u64,
    pub(super) may_activate: bool,
}

/// Keep the objective, optional budget and chosen status separate from the main composer.
pub(super) struct GoalDraft {
    pub(super) objective: TextArea<'static>,
    pub(super) budget: TextArea<'static>,
    pub(super) status: Option<String>,
    pub(super) field: usize,
}

/// Require an explicit confirmation for work that may start immediately or clear saved state.
pub(super) enum GoalConfirm {
    Activate(Value),
    Clear,
}

impl GoalDraft {
    /// Keep status unselected until Save freezes the observed value; new goals begin paused.
    pub(super) fn new(goal: &Value) -> Self {
        let mut objective = TextArea::default();
        objective.insert_str(goal["objective"].as_str().unwrap_or(""));
        Self {
            objective,
            budget: TextArea::default(),
            status: goal.is_null().then(|| "paused".into()),
            field: 0,
        }
    }

    /// Keep a blank existing budget rather than inventing a sentinel or removing it.
    pub(super) fn params(&self, thread: &str) -> Result<Value, String> {
        let objective = self.objective.lines().join("\n");
        let objective = objective.trim();
        if objective.is_empty() {
            return Err("Enter a goal objective".into());
        }
        if objective.len() > 32 * 1024 {
            return Err("Goal objective exceeds32 KiB".into());
        }
        let mut params =
            serde_json::json!({"threadId":thread,"objective":objective,"origin":"user"});
        let budget = self.budget.lines().join("");
        if !budget.trim().is_empty() {
            let value = budget
                .trim()
                .parse::<i64>()
                .ok()
                .filter(|value| *value > 0)
                .ok_or("Token budget must be a positive whole number up to9223372036854775807")?;
            params["tokenBudget"] = serde_json::json!(value);
        }
        if let Some(status) = &self.status {
            params["status"] = serde_json::json!(status);
        }
        Ok(params)
    }
}
