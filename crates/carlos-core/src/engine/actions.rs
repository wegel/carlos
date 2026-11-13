//! Send startup, history and context controls without waiting inside the session owner.

use std::collections::VecDeque;

use serde_json::Value;

use super::owner::Owner;
use crate::backend_commands::CommandHandle;

type ActionCall = Box<dyn FnOnce() -> anyhow::Result<bool> + Send>;
use super::workers::Completion;
use crate::backend::BackendKind;
use crate::session::actions::ServerRequestAction;

pub(super) enum ActionKind {
    NativeControl,
    Settings,
    ReplyError,
    Context,
    ContextRead(u64),
}

#[derive(Default)]
pub(super) struct Actions {
    pending: VecDeque<ServerRequestAction>,
    bytes: usize,
}

impl Owner {
    pub(super) fn action(&mut self, action: ServerRequestAction) {
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
        let commands = match self.client.capture_commands() {
            Ok(commands) => commands,
            Err(error) => {
                self.session
                    .set_status(format!("native control was not sent: {error}"));
                return;
            }
        };
        let generation = commands.connection_generation();
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
        self.workers.actions.submit(move || Completion::Action {
            generation,
            result: call().map_err(|error| error.to_string()),
            kind,
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
            | ServerRequestAction::CodexQueuePage { .. }
            | ServerRequestAction::ValidateCodexForm { .. } => {
                unreachable!("asynchronous owner action")
            }
        }
    }

    pub(super) fn finish_action(&mut self, kind: ActionKind, result: Result<bool, String>) {
        match (kind, result) {
            (ActionKind::Settings, Ok(applied)) => {
                if applied {
                    self.session.invalidate_claude_context();
                    self.session.mark_runtime_settings_applied();
                }
                self.session.refresh_claude_context();
            }
            (ActionKind::NativeControl, Err(error)) => {
                let mut report = Default::default();
                self.session.fail_claude(&mut report);
                self.session
                    .set_status(format!("Claude session failed: {error}"));
                let report = self
                    .session
                    .finish_claude_update(report, &self.session.backend_context());
                self.report(report);
            }
            (ActionKind::ContextRead(token), Err(_)) => {
                self.session
                    .fail_claude_context_request(token, std::time::Instant::now());
            }
            (_, Err(error)) => {
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
        ServerRequestAction::ApplyClaudeSettings | ServerRequestAction::RefreshContext => {
            &Value::Null
        }
    };
    512usize.saturating_add(serde_json::to_vec(value).map_or(usize::MAX - 512, |bytes| bytes.len()))
}
