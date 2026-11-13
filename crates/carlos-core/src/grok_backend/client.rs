//! Map the shared backend interface onto native Grok prompt and reply operations.

use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use serde_json::Value;

use super::GrokClient;
use crate::backend::{BackendClient, BackendKind, EventReceiver};

impl BackendClient for GrokClient {
    fn kind(&self) -> BackendKind {
        BackendKind::Grok
    }

    fn capture_commands(&self) -> Result<crate::backend_commands::CommandHandle> {
        self.capture()
    }

    fn call(&self, method: &str, params: Value, timeout: Duration) -> Result<String> {
        self.commands
            .call_for(&self.current_session_id()?, method, params, timeout)
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
        self.commands
            .stopped
            .store(true, std::sync::atomic::Ordering::SeqCst);
        if let Ok(mut process) = self.process_owner.lock() {
            process.stop();
        }
    }
}

impl Drop for GrokClient {
    fn drop(&mut self) {
        BackendClient::stop(self);
    }
}
