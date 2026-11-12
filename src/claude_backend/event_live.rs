//! Turns native main-conversation events into common items and named activity facts.

use anyhow::Result;
use serde_json::{json, Value};

use super::event_facts::status_fact;
use super::event_history::HistoryDecoder;
use super::event_lifecycle::prompt_ids;
use super::event_normalizer::{fact, EventNormalizer};
use super::event_sources::attach_sources;
use super::message_visibility::{is_child_message, is_child_report_notice};
use super::translate::translate_claude_line;
use super::types::ClaudeTranslationState;

impl EventNormalizer {
    /// Normalize one accepted SDK message after conversation and child fences.
    pub(super) fn sdk(&mut self, message: &Value, epoch: u64) -> Result<Vec<String>> {
        if message["type"] == "rate_limit_event" {
            return Ok(vec![fact(
                "claude/quota",
                message["rate_limit_info"].clone(),
            )]);
        }
        if message["type"] == "system" && message["subtype"] == "elicitation_complete" {
            return Ok(vec![fact("claude/browserComplete", message.clone())]);
        }
        if task_message(message) {
            return Ok(vec![fact("claude/task", message.clone())]);
        }
        if is_child_message(message) || is_child_report_notice(message) {
            return Ok(Vec::new());
        }
        if message["type"] == "conversation_reset" {
            self.epoch = epoch;
            let key = message["new_conversation_id"].as_str().unwrap_or("");
            if self.reset_key.as_deref() == Some(key) {
                return Ok(vec![fact("claude/conversationReset", message.clone())]);
            }
            self.reset_key = Some(key.to_string());
            self.live = ClaudeTranslationState::default();
            self.history = HistoryDecoder::default();
            return Ok(vec![fact("claude/conversationReset", message.clone())]);
        }
        match (message["type"].as_str(), message["subtype"].as_str()) {
            (Some("system"), Some("session_state_changed")) => {
                Ok(vec![fact("claude/activity", message.clone())])
            }
            (Some("result"), _) => {
                self.live.current_turn_id = None;
                Ok(vec![fact(
                    "claude/nativeResult",
                    json!({
                        "user_message_uuids": prompt_ids(message),
                        "subtype":message["subtype"], "is_error":message["is_error"],
                        "terminal_reason":message["terminal_reason"],
                        "error":(message["is_error"] == true || message["subtype"] != "success")
                            .then(|| super::event_facts::result_error(message))
                    }),
                )])
            }
            (Some("system"), Some("status" | "compact_boundary")) => Ok(vec![status_fact(message)]),
            (Some("system"), Some("thinking_tokens")) => Ok(vec![fact(
                "claude/thinking",
                json!({"estimated_tokens":message["estimated_tokens"]}),
            )]),
            _ => self.items(message),
        }
    }

    fn bind_translation(&mut self, message: &Value, output: &mut Vec<String>) {
        if matches!(message["type"].as_str(), Some("stream_event" | "assistant"))
            || message["subtype"] == "init"
        {
            let ids = prompt_ids(message);
            let init = message["type"] == "system" && message["subtype"] == "init";
            if init {
                self.live.current_turn_id = None;
            }
            if let Some(id) = ids.last() {
                self.live.current_turn_id = Some(id.clone());
            }
            if !ids.is_empty() || init {
                output.push(fact(
                    "claude/nativeBind",
                    json!({"user_message_uuids":ids,"init":init}),
                ));
            }
        }
    }

    fn items(&mut self, message: &Value) -> Result<Vec<String>> {
        let mut output = Vec::new();
        if message["type"] == "assistant" && !message["usage_report"].is_null() {
            output.push(fact(
                "claude/usageReport",
                json!({"uuid":message["uuid"],"usage_report":message["usage_report"]}),
            ));
        }
        if message["type"] == "system" {
            let mut notice = message.clone();
            if handled_notice(message) && !message["uuid"].is_string() {
                notice["noticeKey"] = json!(message.to_string());
            }
            output.push(fact("claude/notice", notice));
            if handled_notice(message) {
                return Ok(output);
            }
            if message["subtype"] == "init" {
                output.push(fact(
                    "claude/initialized",
                    json!({"session_id":message["session_id"],"model":message["model"]}),
                ));
            }
        }
        self.bind_translation(message, &mut output);
        if message["event"]["type"] == "content_block_start"
            && !matches!(
                message["event"]["content_block"]["type"].as_str(),
                Some("thinking" | "redacted_thinking")
            )
        {
            output.push(fact("claude/thinking", json!({"estimated_tokens":null})));
        }
        // main ignored malformed SDK records after applying their native side effects.
        let Ok(translated) = translate_claude_line(&mut self.live, &message.to_string()) else {
            return Ok(output);
        };
        for line in translated.lines {
            let mut value: Value = serde_json::from_str(&line)?;
            if matches!(
                value["method"].as_str(),
                Some(
                    "thread/tokenUsage/updated"
                        | "claude/exitPlan/requestApproval"
                        | "turn/started"
                        | "turn/completed"
                )
            ) {
                continue;
            }
            attach_sources(&self.live, message, &mut value);
            output.push(value.to_string());
        }
        Ok(output)
    }
}

fn task_message(message: &Value) -> bool {
    message["type"] == "system"
        && matches!(
            message["subtype"].as_str(),
            Some(
                "task_started"
                    | "task_updated"
                    | "task_progress"
                    | "task_notification"
                    | "background_tasks_changed"
            )
        )
}

fn handled_notice(message: &Value) -> bool {
    matches!(
        message["subtype"].as_str(),
        Some("informational" | "session_writes_lost" | "plugin_install" | "hook_response")
    )
}
