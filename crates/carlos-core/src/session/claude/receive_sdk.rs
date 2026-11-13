//! Apply normalized Claude SDK facts without asking a terminal to interpret them.

use serde_json::Value;

use super::receive_update::{ClaudeEffect, ClaudeUpdate};
use super::CompactionResult;
use crate::session::actions::ServerRequestAction;
use crate::session::models::{MessageKind, Role};
use crate::session::state::SessionState;
use crate::session::text::safe_label;

impl SessionState {
    /// Apply backend-owned facts and return only explicit client changes.
    pub(crate) fn receive_claude_sdk(
        &mut self,
        method: &str,
        facts: &Value,
        update: &mut ClaudeUpdate,
    ) {
        match method {
            "claude/quota" => self.claude_quota_event(facts),
            "claude/browserComplete" => {
                if let Some(callback) = self.receive_claude_browser_completion(facts) {
                    update.effects.push(ClaudeEffect::Browser(callback));
                }
            }
            "claude/task" => {
                if let Err(error) = self.update_claude_task(facts) {
                    self.bounded_claude_failure(error, update);
                }
            }
            "claude/conversationReset" => self.receive_claude_reset(facts, update),
            "claude/activity" => self.record_claude_activity(facts),
            "claude/status" => self.receive_claude_status(facts, update),
            "claude/thinking" => self.record_claude_thinking(facts),
            "claude/result" => {
                if let Some(error) = facts["error"].as_str() {
                    self.append_message(Role::System, error.to_string());
                }
                self.finish_claude_activity();
                update.action = Some(ServerRequestAction::RefreshContext);
            }
            "claude/usageReport" => {
                for (key, text) in self.claude_usage_notice(facts) {
                    self.record_claude_notice(key, &text);
                }
            }
            "claude/notice" => {
                for (key, text) in self.receive_claude_notice(facts) {
                    self.record_claude_notice(key, &text);
                }
            }
            "claude/initialized" => self.receive_claude_initialized(facts, update),
            "claude/promptsRetired" => {
                if let Some(ids) = facts["ids"].as_array() {
                    for id in ids.iter().filter_map(Value::as_str) {
                        self.remove_claude_prompt(id);
                    }
                }
            }
            "claude/promptsConsumed" => self.receive_claude_consumed(facts, update),
            _ => {}
        }
    }

    /// Preserve exact prompt bindings and redraws for consumed native UUIDs.
    pub(crate) fn receive_claude_consumed(&mut self, facts: &Value, update: &mut ClaudeUpdate) {
        if let Some(ids) = facts["ids"].as_array() {
            for id in ids.iter().filter_map(Value::as_str) {
                let key = self.backend_entry_key(&format!("claude-prompt:{id}"));
                let changed = self.consume_claude_prompt(id, &key);

                update.items.reset_navigation |= changed.reset_navigation;
            }
        }
    }

    /// Stop an overflowing helper after retaining its recoverable human work.
    pub(in crate::session) fn bounded_claude_failure(
        &mut self,
        error: &str,
        update: &mut ClaudeUpdate,
    ) {
        self.append_message(Role::System, error.to_string());
        self.fail_claude(update);
        update.action = Some(ServerRequestAction::ClaudeControl {
            method: "shutdown".into(),
            params: serde_json::json!({}),
        });
    }

    fn receive_claude_status(&mut self, facts: &Value, update: &mut ClaudeUpdate) {
        if facts["compaction"] == true {
            self.receive_claude_compaction(facts);
        } else {
            self.record_claude_status(facts);
        }
        if facts["refreshContext"] == true {
            update.action = Some(ServerRequestAction::RefreshContext);
        }
    }

    /// Format the existing per-attempt compaction notice beside its session facts.
    pub(crate) fn receive_claude_compaction(&mut self, facts: &Value) {
        let notice = self.compact_claude_context(facts);
        let text = match notice.result {
            CompactionResult::Failed => format!(
                "Claude could not compact context: {}",
                safe_label(facts["compact_error"].as_str().unwrap_or("unknown error"))
            ),
            CompactionResult::Success => "Claude compacted context".to_string(),
            CompactionResult::Pending => "Claude is compacting context".to_string(),
        };
        let key = self.backend_entry_key(&notice.key);
        self.upsert_mapped_message(&key, Role::System, text, MessageKind::Plain, None);
    }
}
