//! Capture one Claude helper and conversation while its caller keeps lifecycle normalization.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use anyhow::{anyhow, bail, ensure, Context, Result};
use serde_json::{json, Value};

use super::event_queue::EventSender;
use super::transport::ControllerTransport;
use super::types::{normalize_runtime_arg, ClaudeRuntimeSettings};
use super::{sessions, ClaudeClient};
use crate::backend::{BackendKind, PromptOrigin, PromptPriority, RewindForkRequest};
use crate::backend_commands::{BackendCommands, CommandHandle};
use crate::dispatch_guard::DispatchGuard;

#[derive(Clone)]
pub(super) struct Commands {
    permission: DispatchGuard,
    pub(super) transport: Arc<ControllerTransport>,
    pub(super) owner: Weak<Mutex<Arc<ControllerTransport>>>,
    pub(super) events_tx: EventSender,
    pub(super) cwd: PathBuf,
    pub(super) runtime_settings: Arc<Mutex<ClaudeRuntimeSettings>>,
    epoch: u64,
}

impl ClaudeClient {
    pub(super) fn commands(&self) -> Result<Commands> {
        let transport = self
            .transport
            .try_lock()
            .map_err(|_| anyhow!("Claude helper is changing; command not dispatched"))?
            .clone();
        let epoch = transport
            .conversation
            .lock()
            .map_err(|_| anyhow!("Claude conversation lock poisoned"))?
            .epoch;
        Ok(Commands {
            permission: DispatchGuard::default(),
            transport,
            epoch,
            owner: Arc::downgrade(&self.transport),
            events_tx: self.events_tx.clone(),
            cwd: self.cwd.clone(),
            runtime_settings: self.runtime_settings.clone(),
        })
    }
}

impl Commands {
    pub(super) fn write_permission(&self, callback: bool) -> DispatchGuard {
        let captured = self.clone();
        self.permission.clone().and(DispatchGuard::new(move || {
            if callback {
                captured.check_helper().is_ok()
            } else {
                captured.check().is_ok()
            }
        }))
    }

    fn check_helper(&self) -> Result<()> {
        let owner = self.owner.upgrade().context("Claude owner has stopped")?;
        let current = owner
            .lock()
            .map_err(|_| anyhow!("Claude transport lock poisoned"))?;
        ensure!(
            Arc::ptr_eq(&current, &self.transport),
            "captured Claude helper was replaced"
        );
        drop(current);
        Ok(())
    }

    pub(super) fn check(&self) -> Result<()> {
        self.permission.check()?;
        self.check_helper()?;
        ensure!(
            self.transport
                .conversation
                .lock()
                .map_err(|_| anyhow!("Claude conversation lock poisoned"))?
                .epoch
                == self.epoch,
            "captured Claude conversation has reset"
        );
        Ok(())
    }

    pub(super) fn helper_call(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value> {
        self.check()?;
        self.transport
            .call_guarded(method, params, timeout, self.write_permission(false))
    }

    pub(super) fn apply_runtime_settings(
        &self,
        model: Option<&str>,
        effort: Option<&str>,
    ) -> Result<bool> {
        let desired = ClaudeRuntimeSettings {
            model: normalize_runtime_arg(model),
            effort: normalize_runtime_arg(effort).filter(|effort| effort != "auto"),
        };
        let current = self
            .runtime_settings
            .lock()
            .map_err(|_| anyhow!("Claude settings lock poisoned"))?
            .clone();
        if current.model != desired.model {
            self.helper_call(
                "model.set",
                json!({"model":desired.model}),
                Duration::from_secs(10),
            )?;
            self.runtime_settings
                .lock()
                .map_err(|_| anyhow!("Claude settings lock poisoned"))?
                .model = desired.model;
        }
        if current.effort != desired.effort {
            self.helper_call(
                "effort.set",
                json!({"effort":desired.effort}),
                Duration::from_secs(10),
            )?;
            self.runtime_settings
                .lock()
                .map_err(|_| anyhow!("Claude settings lock poisoned"))?
                .effort = desired.effort;
        }
        Ok(true)
    }
}

impl BackendCommands for Commands {
    fn kind(&self) -> BackendKind {
        BackendKind::Claude
    }
    fn connection_generation(&self) -> Option<u64> {
        Some(self.transport.generation)
    }
    fn guarded(&self, permission: DispatchGuard) -> Result<CommandHandle> {
        let mut commands = self.clone();
        commands.permission = commands.permission.and(permission);
        Ok(CommandHandle::new(commands))
    }
    fn call(&self, method: &str, params: Value, timeout: Duration) -> Result<String> {
        let result = match method {
            "thread/name/set" => self.helper_call(
                "sessions.rename",
                json!({"cwd":self.cwd,"sessionId":params["threadId"],"title":params["name"]}),
                timeout,
            )?,
            "turn/interrupt" => self.helper_call("interrupt", json!({}), timeout)?,
            other => bail!("unsupported Claude method: {other}"),
        };
        Ok(json!({"result":result}).to_string())
    }
    fn submit_prompt(
        &self,
        _method: &str,
        params: Value,
        origin: PromptOrigin,
        priority: PromptPriority,
        timeout: Duration,
    ) -> Result<String> {
        self.submit(&params, origin, priority, timeout)
    }
    fn fork_from_rewind(
        &self,
        thread: &str,
        request: RewindForkRequest,
        _timeout: Duration,
    ) -> Result<String> {
        self.check()?;
        let before = request
            .before_turn_id
            .context("Select a prompt with a native UUID before editing")?;
        let settings = self
            .runtime_settings
            .lock()
            .map_err(|_| anyhow!("Claude settings lock poisoned"))?
            .clone();
        sessions::fork_session(
            self.owner.upgrade().context("Claude owner has stopped")?,
            sessions::ForkSource {
                helper: self.transport.clone(),
                epoch: self.epoch,
            },
            self.events_tx.for_generation(self.transport.generation),
            self.cwd.clone(),
            settings,
            thread.to_owned(),
            before,
        );
        Ok(json!({"result":{"claudeForkPending":true}}).to_string())
    }
    fn request_native_control(&self, method: &str, mut params: Value) -> Result<()> {
        self.check()?;
        if method == "history.continue" {
            if params["generation"]
                .as_u64()
                .is_some_and(|generation| generation != self.transport.generation)
            {
                return Ok(());
            }
            return self
                .transport
                .emitter
                .continue_history(params["epoch"].as_u64());
        }
        params["cwd"] = json!(self.cwd);
        if method == "start" {
            let settings = self
                .runtime_settings
                .lock()
                .map_err(|_| anyhow!("Claude settings lock poisoned"))?
                .clone();
            let id = params["sessionId"]
                .as_str()
                .context("Resume has no session ID")?;
            params = super::helper::start_params(
                &self.cwd,
                &super::types::ClaudeLaunchMode::Resume(id.to_string()),
                &settings,
            )?;
        }
        self.transport
            .request_guarded(method, params, self.write_permission(false))
    }
    fn apply_runtime_settings(&self, model: Option<&str>, effort: Option<&str>) -> Result<bool> {
        Commands::apply_runtime_settings(self, model, effort)
    }
    fn refresh_context(&self) -> Result<()> {
        self.request_native_control("context", json!({}))
    }
    fn respond(&self, id: &Value, result: Value) -> Result<()> {
        let id = id.as_str().context("native callback ID must be a string")?;
        self.check_helper()?;
        self.transport.call_guarded(
            "callback.respond",
            json!({"callbackId":id,"answer":result}),
            Duration::from_secs(10),
            self.write_permission(true),
        )?;
        Ok(())
    }
    fn respond_error(&self, id: &Value, _code: i64, message: &str) -> Result<()> {
        bail!("Cannot answer native callback {id}: {message}")
    }
}

pub(super) fn capture(client: &ClaudeClient) -> Result<CommandHandle> {
    Ok(CommandHandle::new(client.commands()?))
}
