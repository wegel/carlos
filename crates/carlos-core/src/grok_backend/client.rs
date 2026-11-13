//! Map the shared backend interface onto native Grok prompt and reply operations.

use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};

use super::types::turn_input_text;
use super::GrokClient;
use crate::backend::{BackendClient, BackendKind, EventReceiver};

impl BackendClient for GrokClient {
    fn kind(&self) -> BackendKind {
        BackendKind::Grok
    }

    fn call(&self, method: &str, params: Value, _timeout: Duration) -> Result<String> {
        match method {
            "turn/start" => {
                self.apply_runtime_settings(
                    params.get("model").and_then(Value::as_str),
                    params.get("effort").and_then(Value::as_str),
                )?;
                let text = turn_input_text(&params)?;
                let session_id = self.current_session_id()?;
                self.begin_prompt_translation()?;
                let process = self
                    .process
                    .lock()
                    .map_err(|_| anyhow!("Grok process lock poisoned"))?;
                process.start_prompt(json!({
                    "sessionId": session_id,
                    "prompt": [{ "type": "text", "text": text }],
                }))?;
                Ok(json!({ "jsonrpc": "2.0", "result": {} }).to_string())
            }
            "turn/steer" => bail!("Grok does not support mid-turn steer"),
            "turn/interrupt" => self.interrupt_session(),
            other => bail!("unsupported Grok backend method: {other}"),
        }
    }

    fn fork_from_rewind(
        &self,
        _thread_id: &str,
        request: crate::backend::RewindForkRequest,
        _timeout: Duration,
    ) -> Result<String> {
        self.rewind_fork(request)
    }

    fn respond(&self, request_id: &Value, _result: Value) -> Result<()> {
        bail!("unsupported Grok approval request: {request_id}")
    }

    fn respond_error(&self, _request_id: &Value, _code: i64, _message: &str) -> Result<()> {
        bail!("Grok backend approvals are not implemented")
    }

    fn take_events_rx(&mut self) -> Result<EventReceiver> {
        self.events_rx
            .take()
            .map(Into::into)
            .ok_or_else(|| anyhow!("Grok events receiver already taken"))
    }

    fn stop(&mut self) {
        if let Ok(mut process) = self.process.lock() {
            process.stop();
        }
    }
}

impl Drop for GrokClient {
    fn drop(&mut self) {
        BackendClient::stop(self);
    }
}
