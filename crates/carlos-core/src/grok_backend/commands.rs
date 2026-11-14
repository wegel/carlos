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
    session: String,
    revision: u64,
}

impl GrokClient {
    pub(super) fn capture(&self) -> Result<CommandHandle> {
        Ok(CommandHandle::new(Captured {
            permission: DispatchGuard::default(),
            commands: Arc::downgrade(&self.commands),
            session: self.current_session_id()?,
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
                self.begin_prompt_translation()?;
                let process = &self.process;
                process.start_prompt(
                    json!({
                        "sessionId": session_id,
                        "prompt": [{ "type": "text", "text": text }],
                    }),
                    permission,
                )?;
                Ok(json!({ "jsonrpc": "2.0", "result": {} }).to_string())
            }
            "turn/steer" => bail!("Grok does not support mid-turn steer"),
            "turn/interrupt" => self.interrupt_session(session_id, permission),
            other => bail!("unsupported Grok backend method: {other}"),
        }
    }
}

impl Captured {
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
                && commands.revision.load(Ordering::SeqCst) == self.revision
                && commands.current_session_id()? == self.session,
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
            &self.session,
            method,
            params,
            timeout,
            self.write_permission(),
        )
    }
    fn respond(&self, id: &Value, _result: Value) -> Result<()> {
        bail!("unsupported Grok approval request: {id}")
    }
    fn respond_error(&self, _id: &Value, _code: i64, _message: &str) -> Result<()> {
        bail!("Grok backend approvals are not implemented")
    }
}
