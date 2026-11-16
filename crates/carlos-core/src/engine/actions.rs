//! Send startup, history and context controls without waiting inside the session owner.

use std::collections::VecDeque;

use serde_json::Value;

use super::owner::Owner;
use crate::backend_commands::CommandHandle;

use super::target::Target;
use super::workers::Completion;

type ActionCall = Box<dyn FnOnce() -> anyhow::Result<bool> + Send>;
pub(super) enum ActionFailure {
    Retired,
    Native(String),
}
use crate::backend::BackendKind;
use crate::session::actions::ServerRequestAction;
use crate::session::claude::QuotaReadKey;

pub(super) enum ActionKind {
    NativeControl,
    Settings,
    ReplyError,
    Context,
    ContextRead(u64),
    QuotaRead(QuotaReadKey),
}

#[derive(Default)]
pub(super) struct Actions {
    pending: VecDeque<ServerRequestAction>,
    bytes: usize,
}

impl Actions {
    pub(super) fn retire(&mut self) {
        self.pending.clear();
        self.bytes = 0;
    }
}

impl Owner {
    pub(super) fn action(&mut self, action: ServerRequestAction) {
        if matches!(action, ServerRequestAction::ApplyRalphToggle) {
            // The ordinary dispatcher applies the existing pending flag when its worker is free.
            return;
        }
        let bytes = size(&action);
        if self.actions.pending.len() >= 64
            || bytes > (8 * 1024 * 1024usize).saturating_sub(self.actions.bytes)
        {
            self.session.events.backend_closed = true;
            self.session.ralph_stop_after_failure();
            self.session.set_status(
                "native action backlog exceeded its budget; session stopped without retry",
            );
            self.channels
                .stop
                .store(true, std::sync::atomic::Ordering::SeqCst);
            return;
        }
        self.actions.bytes += bytes;
        self.actions.pending.push_back(action);
        let _ = self.channels.wake.try_send(());
    }

    pub(super) fn quota_action(&mut self, params: Value) {
        let key = QuotaReadKey::from_context(&params).expect("prepared quota request");
        let action = ServerRequestAction::ClaudeControl {
            method: "usage.optional".into(),
            params,
        };
        if self.actions.pending.len() >= 64
            || size(&action) > (8 * 1024 * 1024usize).saturating_sub(self.actions.bytes)
        {
            self.session.fail_claude_quota_request(key);
            return;
        }
        self.action(action);
    }

    pub(super) fn dispatch_actions(&mut self) {
        while self.workers.actions_busy < 2 {
            let Some(action) = self.actions.pending.pop_front() else {
                break;
            };
            self.actions.bytes -= size(&action);
            match action {
                ServerRequestAction::CodexQueuePage { .. }
                | ServerRequestAction::ValidateCodexForm { .. } => {
                    // These native APIs already enqueue tokenized reads without waiting for writes or replies.
                    let context = self.session.backend_context();
                    let report =
                        self.session
                            .run_backend_action(self.client.as_ref(), action, &context);
                    self.report(report);
                }
                ServerRequestAction::RefreshContext
                    if self.client.kind() == BackendKind::Claude =>
                {
                    self.session.refresh_claude_context()
                }
                action => self.start_action(action),
            }
        }
    }

    fn start_action(&mut self, action: ServerRequestAction) {
        let quota = quota_key(&action);
        if quota.is_some_and(|key| !self.session.claude_quota_request_current(key)) {
            return;
        }
        let commands = match self.client.capture_commands() {
            Ok(commands) => commands,
            Err(error) => {
                if let Some(key) = quota {
                    self.session.fail_claude_quota_request(key);
                    return;
                }
                if let ServerRequestAction::ClaudeControl { method, params } = &action {
                    if method == "context" {
                        self.session.fail_claude_context_request(
                            params["contextToken"].as_u64().unwrap_or(0),
                            std::time::Instant::now(),
                        );
                        return;
                    }
                }
                self.session
                    .set_status(format!("native control was not sent: {error}"));
                return;
            }
        };
        let target = Target::capture(&self.session, self.client.kind());
        let captured = commands.clone();
        if let ServerRequestAction::ReplyNative { card_id, result } = action {
            if let Some(work) =
                self.session
                    .prepare_checked_native_answer(self.client.kind(), card_id, result)
            {
                self.workers.actions_busy += 1;
                self.workers
                    .actions
                    .submit(move || Completion::InternalControl(work.execute(&commands)));
            }
            return;
        }
        let (kind, call) = self.action_call(action, commands);
        self.workers.actions_busy += 1;
        self.workers.actions.submit(move || {
            let result = call();
            let result = if captured.is_current() {
                result.map_err(|error| ActionFailure::Native(error.to_string()))
            } else {
                Err(ActionFailure::Retired)
            };
            Completion::Action {
                target,
                result,
                kind,
            }
        });
    }

    fn action_call(
        &self,
        action: ServerRequestAction,
        commands: CommandHandle,
    ) -> (ActionKind, ActionCall) {
        match action {
            ServerRequestAction::ClaudeControl { method, params } => {
                let kind = if method == "context" {
                    ActionKind::ContextRead(params["contextToken"].as_u64().unwrap_or(0))
                } else if method == "usage.optional" {
                    ActionKind::QuotaRead(
                        QuotaReadKey::from_context(&params).expect("prepared quota request"),
                    )
                } else {
                    ActionKind::NativeControl
                };
                (
                    kind,
                    Box::new(move || {
                        commands
                            .request_native_control(&method, params)
                            .map(|()| false)
                    }),
                )
            }
            ServerRequestAction::ApplyClaudeSettings => {
                let (model, effort, _) = self.session.next_turn_runtime_settings();
                let (model, effort) = self.session.claude_prompt_choices(model, effort);
                (
                    ActionKind::Settings,
                    Box::new(move || {
                        commands.apply_runtime_settings(model.as_deref(), effort.as_deref())
                    }),
                )
            }
            ServerRequestAction::ReplyError {
                request_id,
                code,
                message,
            } => (
                ActionKind::ReplyError,
                Box::new(move || {
                    commands
                        .respond_error(&request_id, code, &message)
                        .map(|()| false)
                }),
            ),
            ServerRequestAction::RefreshContext => (
                ActionKind::Context,
                Box::new(move || commands.refresh_context().map(|()| false)),
            ),
            ServerRequestAction::ReplyNative { .. }
            | ServerRequestAction::ApplyRalphToggle
            | ServerRequestAction::CodexQueuePage { .. }
            | ServerRequestAction::ValidateCodexForm { .. } => {
                unreachable!("asynchronous owner action")
            }
        }
    }

    pub(super) fn finish_action(&mut self, kind: ActionKind, result: Result<bool, ActionFailure>) {
        match (kind, result) {
            (ActionKind::Settings, Ok(applied)) => {
                if applied {
                    self.session.invalidate_claude_context();
                    self.session.mark_runtime_settings_applied();
                }
                self.session.refresh_claude_context();
            }
            (ActionKind::NativeControl, Err(ActionFailure::Native(error))) => {
                let mut report = Default::default();
                self.session.fail_claude(&mut report);
                self.session
                    .set_status(format!("Claude session failed: {error}"));
                let report = self
                    .session
                    .finish_claude_update(report, &self.session.backend_context());
                self.report(report);
            }
            (ActionKind::QuotaRead(key), Err(_)) => {
                self.session.fail_claude_quota_request(key);
            }
            (ActionKind::ContextRead(token), Err(_)) => {
                self.session
                    .fail_claude_context_request(token, std::time::Instant::now());
            }
            (_, Err(ActionFailure::Retired)) => {}
            (_, Err(ActionFailure::Native(error))) => {
                self.session.ralph_stop_after_failure();
                self.session
                    .set_status(format!("native control failed: {error}"));
            }
            (_, Ok(_)) => {}
        }
    }
}

fn size(action: &ServerRequestAction) -> usize {
    let value = match action {
        ServerRequestAction::ClaudeControl { params, .. } => params,
        ServerRequestAction::ReplyNative { result, .. } => result,
        ServerRequestAction::CodexQueuePage { cursor, .. } => cursor,
        ServerRequestAction::ValidateCodexForm { schema, .. } => schema,
        ServerRequestAction::ReplyError { request_id, .. } => request_id,
        ServerRequestAction::ApplyClaudeSettings
        | ServerRequestAction::ApplyRalphToggle
        | ServerRequestAction::RefreshContext => &Value::Null,
    };
    512usize.saturating_add(serde_json::to_vec(value).map_or(usize::MAX - 512, |bytes| bytes.len()))
}

fn quota_key(action: &ServerRequestAction) -> Option<QuotaReadKey> {
    match action {
        ServerRequestAction::ClaudeControl { method, params } if method == "usage.optional" => {
            QuotaReadKey::from_context(params)
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "context_capture_tests.rs"]
mod context_capture_tests;
