//! Fences native conversations while preserving process-level tasks and actual callback IDs.

use std::collections::VecDeque;

use anyhow::{bail, Result};
use serde_json::Value;

/// Grants one observed reset permission to adopt the next native init identity.
#[derive(Default)]
pub(super) struct ConversationGuard {
    pub(super) expected: Option<String>,
    pub(super) epoch: u64,
    permit: bool,
    resets: VecDeque<String>,
    held: Vec<Value>,
    held_bytes: usize,
}

impl ConversationGuard {
    /// Classify complete SDK frames without assigning callback lifetimes to transcript epochs.
    pub(super) fn route(&mut self, frame: Value) -> Result<Vec<(Value, u64)>> {
        if frame["event"] != "sdk" {
            return Ok(vec![(frame, self.epoch)]);
        }
        let message = &frame["message"];
        if message["type"] == "conversation_reset" {
            return self.reset(frame);
        }
        if message["type"] == "system" && message["subtype"] == "init" {
            return self.initialize(frame);
        }
        if process_message(message) {
            return Ok(vec![(frame, self.epoch)]);
        }
        if self.permit {
            self.held_bytes += frame.to_string().len();
            if self.held_bytes > 8 * 1024 * 1024 || self.held.len() >= 1024 {
                bail!("Claude reset frames exceeded the bounded transition buffer");
            }
            self.held.push(frame);
            return Ok(Vec::new());
        }
        Ok(self
            .matches(message)
            .then_some((frame, self.epoch))
            .into_iter()
            .collect())
    }

    fn reset(&mut self, frame: Value) -> Result<Vec<(Value, u64)>> {
        let message = &frame["message"];
        let Some(id) = message["uuid"].as_str() else {
            bail!("Claude reset has no event UUID");
        };
        if self.resets.iter().any(|seen| seen == id)
            || message["session_id"].as_str() != self.expected.as_deref()
            || self.expected.is_none()
        {
            return Ok(Vec::new());
        }
        if id.is_empty() || id.len() > 4096 {
            bail!("Claude reset has an invalid event identity");
        }
        if self.permit
            || !message["new_conversation_id"]
                .as_str()
                .is_some_and(|key| !key.is_empty() && key.len() <= 4096)
        {
            bail!("Claude reset arrived without a complete native identity transition");
        }
        self.resets.push_back(id.to_string());
        if self.resets.len() > 64 {
            self.resets.pop_front();
        }
        self.epoch += 1;
        self.permit = true;
        Ok(vec![(frame, self.epoch)])
    }

    fn initialize(&mut self, frame: Value) -> Result<Vec<(Value, u64)>> {
        let actual = frame["message"]["session_id"]
            .as_str()
            .filter(|id| uuid::Uuid::parse_str(id).is_ok())
            .ok_or_else(|| anyhow::anyhow!("Claude init has no valid session UUID"))?;
        if !self.permit
            && self
                .expected
                .as_deref()
                .is_some_and(|expected| expected != actual)
        {
            bail!(
                "Claude resumed {actual} but {} was requested; session stopped",
                self.expected.as_deref().unwrap_or("unknown")
            );
        }
        self.expected = Some(actual.to_string());
        self.permit = false;
        let mut frames = vec![(frame, self.epoch)];
        let held = std::mem::take(&mut self.held);
        self.held_bytes = 0;
        frames.extend(
            held.into_iter()
                .filter(|frame| self.matches(&frame["message"]))
                .map(|frame| (frame, self.epoch)),
        );
        Ok(frames)
    }

    fn matches(&self, message: &Value) -> bool {
        message["session_id"]
            .as_str()
            .is_none_or(|actual| self.expected.as_deref().is_none_or(|id| id == actual))
    }
}

/// Native tasks and child callbacks belong to the process even when its transcript resets.
pub(crate) fn process_message(message: &Value) -> bool {
    message["agent_id"].is_string()
        || !message["parent_tool_use_id"].is_null()
        || matches!(
            message["subtype"].as_str(),
            Some(
                "background_tasks_changed"
                    | "task_started"
                    | "task_updated"
                    | "task_progress"
                    | "task_notification"
                    | "elicitation_complete"
            )
        )
}
