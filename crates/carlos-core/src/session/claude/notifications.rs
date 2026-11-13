//! Fence named and common Claude facts before any transcript or callback writes.

use serde_json::{Map, Value};

use super::state::ClaudePhase;
use crate::session::state::SessionState;

/// Tell the app which existing handler may consume a normalized line.
pub enum ClaudeRoute {
    Continue,
    Ignore,
    Named,
    Context,
    History,
}

impl SessionState {
    /// Preserve the distinct common-message generation, epoch, and closed guards.
    pub(crate) fn route_claude_notification(
        &mut self,
        method: &str,
        params: &Map<String, Value>,
    ) -> ClaudeRoute {
        if method.starts_with("claude/") && named_fact(method) {
            return ClaudeRoute::Named;
        }
        if !self.claude.enabled {
            return ClaudeRoute::Continue;
        }
        let obsolete_generation = params
            .get("generation")
            .and_then(Value::as_u64)
            .is_some_and(|generation| generation != self.claude.generation);
        let obsolete_epoch = params
            .get("epoch")
            .and_then(Value::as_u64)
            .is_some_and(|epoch| epoch < self.claude.epoch);
        if obsolete_generation || (obsolete_epoch && !params.contains_key("claudeContext")) {
            return ClaudeRoute::Ignore;
        }
        if self.claude.phase == ClaudePhase::Closed && params.contains_key("generation") {
            return ClaudeRoute::Ignore;
        }
        if method == "thread/tokenUsage/updated" && params.contains_key("claudeContext") {
            return ClaudeRoute::Context;
        }
        if method == "turn/completed" && params.contains_key("generation") {
            if let Some(id) = params.get("turn").and_then(|turn| turn["id"].as_str()) {
                self.remove_claude_prompt(id);
            }
        }
        if params.get("history") == Some(&Value::Bool(true)) {
            return ClaudeRoute::History;
        }
        ClaudeRoute::Continue
    }

    /// Accept a replacement only with a newer numeric generation and a string session id.
    pub(crate) fn claude_session_change<'a>(
        &self,
        params: &'a Map<String, Value>,
    ) -> Option<(u64, &'a str)> {
        let generation = params.get("generation").and_then(Value::as_u64)?;
        let id = params.get("sessionId").and_then(Value::as_str)?;
        (generation > self.claude.generation).then_some((generation, id))
    }

    /// Keep process events across epochs, then advance reset epochs only after all gates.
    pub fn accept_claude_fact(&mut self, method: &str, params: &Map<String, Value>) -> bool {
        let generation = params.get("generation").and_then(Value::as_u64);
        if generation.is_some_and(|generation| generation != self.claude.generation) {
            return false;
        }
        let process = matches!(
            method,
            "claude/callback"
                | "claude/callbackCancelled"
                | "claude/task"
                | "claude/browserComplete"
        ) || (method == "claude/controlReply"
            && matches!(
                params.get("requestMethod").and_then(Value::as_str),
                Some("start" | "capabilities" | "task.stop" | "usage.optional" | "context")
            ));
        if params
            .get("epoch")
            .and_then(Value::as_u64)
            .is_some_and(|epoch| epoch < self.claude.epoch)
            && !process
        {
            return false;
        }
        if self.claude.phase == ClaudePhase::Closed
            && !matches!(
                method,
                "claude/error"
                    | "claude/closed"
                    | "claude/stderr"
                    | "claude/callbackCancelled"
                    | "claude/promptSubmitted"
                    | "claude/promptRejected"
                    | "claude/sessionStartFailed"
                    | "claude/forkFailed"
            )
        {
            return false;
        }
        if method == "claude/conversationReset" {
            self.claude.epoch = params
                .get("epoch")
                .and_then(Value::as_u64)
                .unwrap_or(self.claude.epoch);
        }
        true
    }
}

fn named_fact(method: &str) -> bool {
    matches!(
        method,
        "claude/sessionChanged"
            | "claude/sessionStartFailed"
            | "claude/forkFailed"
            | "claude/promptSubmitted"
            | "claude/promptRejected"
            | "claude/controlReply"
            | "claude/callback"
            | "claude/callbackCancelled"
            | "claude/historyRecord"
            | "claude/historyFactPart"
            | "claude/historyPrompt"
            | "claude/historyCompacted"
            | "claude/stderr"
            | "claude/error"
            | "claude/closed"
            | "claude/quota"
            | "claude/browserComplete"
            | "claude/task"
            | "claude/conversationReset"
            | "claude/activity"
            | "claude/status"
            | "claude/thinking"
            | "claude/result"
            | "claude/usageReport"
            | "claude/notice"
            | "claude/initialized"
            | "claude/promptsConsumed"
            | "claude/promptsRetired"
    )
}
