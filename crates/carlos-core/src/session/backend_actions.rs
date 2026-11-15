//! Execute the existing backend replies and controls beside the session facts they change.

use anyhow::Result;
use serde_json::Value;

use super::actions::ServerRequestAction;
use super::backend_update::{BackendChange, BackendContext, BackendUpdate};
use super::codex::callback_update::{CallbackEffect, CallbackUpdate};
use super::codex::queue::QueueUpdate;
use super::state::SessionState;
use super::text::safe_text;
use crate::backend::{BackendClient, BackendKind};

impl SessionState {
    /// Run one accepted action after its caller applies the matching message report.
    pub fn run_backend_action(
        &mut self,
        client: &dyn BackendClient,
        action: ServerRequestAction,
        _context: &BackendContext<'_>,
    ) -> BackendUpdate {
        let change = match action {
            ServerRequestAction::ApplyRalphToggle => self.apply_inline_ralph_toggle(),
            ServerRequestAction::CodexQueuePage { token, cursor } => {
                let mut update = QueueUpdate::default();
                super::codex::queue::page(
                    client,
                    self,
                    self.codex_scope(),
                    &mut update,
                    token,
                    cursor,
                );
                BackendChange::Codex(CallbackUpdate::new(CallbackEffect::Queue(update)))
            }
            ServerRequestAction::ValidateCodexForm {
                id,
                schema,
                answer,
                revision,
            } => match client.validate_form(id.clone(), schema, answer, revision) {
                Ok(()) => BackendChange::None,
                Err(error) => {
                    BackendChange::Codex(self.codex_callback_failure(&id, &error.to_string()))
                }
            },
            ServerRequestAction::ReplyNative { card_id, result } => {
                self.send_backend_native_reply(client, card_id, result)
            }
            ServerRequestAction::ClaudeControl { method, params } => {
                self.send_backend_claude_control(client, &method, params)
            }
            ServerRequestAction::ApplyClaudeSettings => {
                self.apply_backend_claude_settings(client);
                BackendChange::None
            }
            ServerRequestAction::ReplyError {
                request_id,
                code,
                message,
            } => {
                if let Err(error) = client.respond_error(&request_id, code, &message) {
                    self.set_status(format!("server request reply failed: {error}"));
                }
                BackendChange::None
            }
            ServerRequestAction::RefreshContext => {
                if let Err(error) = self.refresh_backend_context(client) {
                    self.set_status(format!("Could not read context: {error}"));
                }
                BackendChange::None
            }
        };
        BackendUpdate::new(change)
    }

    fn apply_inline_ralph_toggle(&mut self) -> BackendChange {
        if let Err(error) = self.apply_pending_ralph_toggle() {
            self.set_status(format!("ralph: {error}"));
        }
        BackendChange::None
    }

    /// Retain the submitted callback and show its exact worker/reply failure locally.
    pub(crate) fn codex_callback_failure(&mut self, id: &Value, error: &str) -> CallbackUpdate {
        self.codex_callback_reply_failed(id);
        let error = safe_text(error);
        self.set_status(error.clone());
        CallbackUpdate::new(CallbackEffect::ReplyFailed {
            id: id.clone(),
            error,
        })
    }

    fn send_backend_native_reply(
        &mut self,
        client: &dyn BackendClient,
        card_id: super::entry_id::EntryId,
        answer: Value,
    ) -> BackendChange {
        let Some(pending) = self.submitted_approval_answer(card_id) else {
            return BackendChange::None;
        };
        let id = pending.request_id.clone();
        let update = match self.send_codex_callback(client, card_id, answer) {
            Ok(Some(retired)) => CallbackUpdate::new(CallbackEffect::Retired(retired)),
            Ok(None) => CallbackUpdate::new(CallbackEffect::None),
            Err(error) => self.codex_callback_failure(&id, &error),
        };
        BackendChange::Codex(update)
    }

    fn apply_backend_claude_settings(&mut self, client: &dyn BackendClient) {
        let (model, effort, _) = self.next_turn_runtime_settings();
        let (model, effort) = self.claude_prompt_choices(model, effort);
        match client.apply_runtime_settings(model.as_deref(), effort.as_deref()) {
            Ok(true) => {
                self.invalidate_claude_context();
                self.mark_runtime_settings_applied();
            }
            Ok(false) => {}
            Err(error) => {
                self.ralph_stop_after_failure();
                self.set_status(format!("Could not apply Claude settings: {error}"));
            }
        }
        if let Err(error) = self.refresh_backend_context(client) {
            self.set_status(format!("Could not read context: {error}"));
        }
    }

    fn refresh_backend_context(&mut self, client: &dyn BackendClient) -> Result<()> {
        if client.kind() == BackendKind::Claude {
            self.refresh_claude_context();
            Ok(())
        } else {
            client.refresh_context()
        }
    }

    fn send_backend_claude_control(
        &mut self,
        client: &dyn BackendClient,
        method: &str,
        params: Value,
    ) -> BackendChange {
        if let Err(error) = client.request_native_control(method, params) {
            let mut update = Default::default();
            self.fail_claude(&mut update);
            self.set_status(format!("Claude session failed: {error}"));
            BackendChange::Claude(update)
        } else {
            BackendChange::None
        }
    }
}
