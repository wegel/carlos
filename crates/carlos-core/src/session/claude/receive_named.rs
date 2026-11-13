//! Handle named Claude JSON messages after the backend's normalized stream fences.

use serde_json::{json, Value};

use super::receive_update::{ClaudeDraftContext, ClaudeEffect, ClaudeUpdate};
use super::ClaudeRoute;
use crate::session::actions::ServerRequestAction;
use crate::session::models::Role;
use crate::session::state::SessionState;

impl SessionState {
    /// Apply a normalized Claude frame or let the common reducer handle it.
    pub fn receive_claude_message(
        &mut self,
        method: &str,
        facts: &Value,
        draft: &ClaudeDraftContext<'_>,
    ) -> Option<ClaudeUpdate> {
        let params = facts.as_object()?;
        let mut update = ClaudeUpdate::default();
        match self.route_claude_notification(method, params) {
            ClaudeRoute::Continue => return None,
            ClaudeRoute::Ignore => {}
            ClaudeRoute::Named => self.receive_named_claude(method, facts, draft, &mut update),
            ClaudeRoute::Context => self.receive_claude_context(
                &json!({"ok":true,"result":params["claudeContext"]}),
                params.get("requestContext").unwrap_or(&Value::Null),
            ),
            ClaudeRoute::History => {
                let previous = self.set_summary_replaying(true);
                crate::session::notification_items::handle_item_notification(
                    self,
                    &mut update.items,
                    method,
                    params,
                );
                self.set_summary_replaying(previous);
            }
        }
        Some(update)
    }

    /// Apply one named helper fact, including startup failures before a session switch.
    pub fn receive_named_claude(
        &mut self,
        method: &str,
        facts: &Value,
        draft: &ClaudeDraftContext<'_>,
        update: &mut ClaudeUpdate,
    ) {
        let Some(params) = facts.as_object() else {
            return;
        };
        self.enable_claude();
        if method == "claude/sessionChanged" {
            if let Some((generation, id)) = self.claude_session_change(params) {
                self.switch_claude_session(generation, id, update);
            }
            return;
        }
        if !self.accept_claude_fact(method, params) {
            return;
        }
        match method {
            "claude/sessionStartFailed" | "claude/forkFailed" => {
                self.claude_startup_failure(method, facts, draft, update)
            }
            "claude/promptSubmitted" => self.receive_claude_accepted(facts, draft, update),
            "claude/promptRejected" => {
                if let Some(id) = params["uuid"].as_str() {
                    self.remove_claude_prompt(id);
                }
            }
            "claude/controlReply" => self.receive_claude_control(
                facts,
                params.get("requestMethod").and_then(Value::as_str),
                params.get("requestContext").unwrap_or(&Value::Null),
                draft,
                update,
            ),
            "claude/callback" => update
                .effects
                .push(ClaudeEffect::Callback(self.receive_claude_callback(facts))),
            "claude/callbackCancelled" => {
                self.receive_claude_cancelled(&params["callbackId"], draft, update)
            }
            "claude/historyFactPart" => self.receive_claude_fragment(facts, draft, update),
            "claude/historyRecord" => self.record_claude_history(params["record"].clone()),
            "claude/historyPrompt" => self.receive_claude_history_prompt(facts, update),
            "claude/historyCompacted" => {
                self.append_message(Role::System, "Conversation compacted");
            }
            "claude/stderr" | "claude/error" | "claude/closed" => {
                self.receive_terminal_claude(method, facts, draft, update)
            }
            _ => self.receive_claude_sdk(method, facts, draft, update),
        }
    }

    fn receive_claude_accepted(
        &mut self,
        facts: &Value,
        draft: &ClaudeDraftContext<'_>,
        update: &mut ClaudeUpdate,
    ) {
        let params = facts.as_object().expect("named Claude object");
        if params["uuid"].is_string() {
            if let Err(error) = self.register_claude_prompt(facts) {
                self.bounded_claude_failure(error, draft, update);
            }
        }
    }

    /// Adopt one classified history prompt and return its original redraw/cursor changes.
    pub(crate) fn receive_claude_history_prompt(
        &mut self,
        facts: &Value,
        update: &mut ClaudeUpdate,
    ) {
        let uuid = facts["uuid"].as_str().unwrap_or("");
        let key = self.backend_entry_key(&format!("claude-prompt:{uuid}"));
        let changed = self.append_claude_history_prompt(facts, &key);

        update.items.reset_navigation |= changed.reset_navigation;
    }

    fn receive_terminal_claude(
        &mut self,
        method: &str,
        facts: &Value,
        draft: &ClaudeDraftContext<'_>,
        update: &mut ClaudeUpdate,
    ) {
        let changed = self.receive_claude_event(method, facts);

        if changed.fail_session {
            self.fail_claude(draft, update);
        }
        if changed.closed {
            self.record_claude_closed(facts);
        }
    }

    fn claude_startup_failure(
        &mut self,
        method: &str,
        facts: &Value,
        draft: &ClaudeDraftContext<'_>,
        update: &mut ClaudeUpdate,
    ) {
        let start = method == "claude/sessionStartFailed";
        if start {
            self.fail_claude(draft, update);
        }
        let message = facts["message"].as_str().unwrap_or(if start {
            "Claude could not start the edited branch"
        } else {
            "Could not fork Claude session"
        });
        self.fail_claude_edit(draft, update, message);
    }

    fn receive_claude_fragment(
        &mut self,
        facts: &Value,
        draft: &ClaudeDraftContext<'_>,
        update: &mut ClaudeUpdate,
    ) {
        match self.receive_claude_history_part(facts) {
            Ok(value) => update.history_frame = value,
            Err(error) => {
                self.record_claude_error(&json!({"message":error.to_string()}));
                self.fail_claude(draft, update);
                return;
            }
        }
        update.action = Some(ServerRequestAction::ClaudeControl {
            method: "history.continue".into(),
            params: json!({"generation":facts["generation"],"epoch":facts["epoch"]}),
        });
    }

    /// Cancel the exact callback and clear only the client's matching consent token.
    pub fn receive_claude_cancelled(
        &mut self,
        id: &Value,
        draft: &ClaudeDraftContext<'_>,
        update: &mut ClaudeUpdate,
    ) {
        let clear_confirmation = draft.confirmation.is_some_and(|entry| {
            self.approval()
                .live()
                .any(|card| card.entry_id() == entry && &card.facts().request_id == id)
        });
        update.effects.push(ClaudeEffect::Cancelled {
            id: id.as_str().map(str::to_string),
            clear_confirmation,
        });
        let (retired, cleared) = self.cancel_claude_callback(id);
        update.effects.push(ClaudeEffect::Cards(retired));
        if let Some(cleared) = cleared {
            update.effects.push(ClaudeEffect::Cleared(cleared));
        }
    }
}
