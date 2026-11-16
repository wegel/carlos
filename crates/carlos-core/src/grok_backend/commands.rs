//! Send ACP commands through a captured session without owning its process or event receiver.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Weak};
use std::time::Duration;

use anyhow::{bail, ensure, Context, Result};
use serde_json::{json, Value};

use super::types::turn_input_text;
use super::{GrokClient, GrokCommands};
use crate::backend::BackendKind;
use crate::backend_commands::{BackendCommands, CommandHandle};
use crate::dispatch_guard::DispatchGuard;

#[derive(Clone)]
struct Captured {
    permission: DispatchGuard,
    commands: Weak<GrokCommands>,
    session: Option<String>,
    revision: u64,
}

impl GrokClient {
    pub(super) fn capture(&self) -> Result<CommandHandle> {
        Ok(CommandHandle::new(Captured {
            permission: DispatchGuard::default(),
            commands: Arc::downgrade(&self.commands),
            session: self
                .session_id
                .lock()
                .map_err(|_| anyhow::anyhow!("Grok session lock poisoned"))?
                .clone(),
            revision: self.commands.revision.load(Ordering::SeqCst),
        }))
    }
}

impl GrokCommands {
    pub(super) fn call_for(
        &self,
        session_id: &str,
        method: &str,
        params: Value,
        _timeout: Duration,
        permission: DispatchGuard,
    ) -> Result<String> {
        match method {
            "turn/start" => {
                self.apply_runtime_settings(
                    session_id,
                    params.get("model").and_then(Value::as_str),
                    params.get("effort").and_then(Value::as_str),
                    permission.clone(),
                )?;
                let text = turn_input_text(&params)?;
                let turn_id = self.begin_prompt_translation()?;
                let process = &self.process;
                process.start_prompt(
                    json!({
                        "sessionId": session_id,
                        "prompt": [{ "type": "text", "text": text }],
                    }),
                    permission,
                )?;
                Ok(json!({ "jsonrpc": "2.0", "result": {"turn":{"id":turn_id}} }).to_string())
            }
            "turn/steer" => bail!("Grok does not support mid-turn steer"),
            "turn/interrupt" => self.interrupt_session(
                session_id,
                params.get("turnId").and_then(Value::as_str),
                permission,
            ),
            other => bail!("unsupported Grok backend method: {other}"),
        }
    }
}

impl Captured {
    fn respond_native(&self, token: &Value, body: Value, error: bool) -> Result<()> {
        let commands = self.current()?;
        let captured = self.clone();
        commands.process.respond_guarded(
            token,
            body,
            error,
            self.write_permission(),
            move |session, epoch| {
                let Ok(commands) = captured.current() else {
                    return false;
                };
                let session_matches = commands.session_id.lock().is_ok_and(|current| match current
                    .as_deref()
                {
                    Some(current) => session == Some(current),
                    None => {
                        captured.session.is_none()
                            && commands.opening.load(Ordering::SeqCst)
                            && session.is_some()
                    }
                });
                session_matches
                    && commands
                        .translation
                        .lock()
                        .is_ok_and(|state| state.prompt_epoch == epoch)
            },
        )
    }

    fn write_permission(&self) -> DispatchGuard {
        let captured = self.clone();
        self.permission
            .clone()
            .and(DispatchGuard::new(move || captured.current().is_ok()))
    }

    fn current(&self) -> Result<Arc<GrokCommands>> {
        let commands = self.commands.upgrade().context("Grok owner has stopped")?;
        ensure!(
            !commands.stopped.load(Ordering::SeqCst)
                && match &self.session {
                    Some(session) =>
                        commands.revision.load(Ordering::SeqCst) == self.revision
                            && commands.current_session_id()? == *session,
                    None =>
                        commands.revision.load(Ordering::SeqCst) <= self.revision.saturating_add(1),
                },
            "captured Grok session has changed"
        );
        Ok(commands)
    }
}

impl BackendCommands for Captured {
    fn kind(&self) -> BackendKind {
        BackendKind::Grok
    }
    fn is_current(&self) -> bool {
        self.current().is_ok()
    }
    fn abort(&self) {
        if let Some(commands) = self.commands.upgrade() {
            commands.process.cancel();
        }
    }
    fn guarded(&self, permission: DispatchGuard) -> Result<CommandHandle> {
        let mut commands = self.clone();
        commands.permission = commands.permission.and(permission);
        Ok(CommandHandle::new(commands))
    }
    fn call(&self, method: &str, params: Value, timeout: Duration) -> Result<String> {
        self.current()?.call_for(
            self.session
                .as_deref()
                .context("Grok session is not ready")?,
            method,
            params,
            timeout,
            self.write_permission(),
        )
    }
    fn respond(&self, id: &Value, result: Value) -> Result<()> {
        self.respond_native(id, result, false)
    }
    fn respond_error(&self, id: &Value, code: i64, message: &str) -> Result<()> {
        self.respond_native(id, json!({"code":code,"message":message}), true)
    }
}
