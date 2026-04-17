//! Backend trait abstracting over codex and claude CLI transports.

use std::sync::mpsc;
use std::time::Duration;

use anyhow::{bail, Result};
use serde_json::Value;

/// Identifies which backend implementation is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BackendKind {
    Codex,
    Claude,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RewindForkRequest {
    pub(crate) keep_turns: usize,
    pub(crate) drop_turns: usize,
}

pub(crate) trait BackendClient {
    fn kind(&self) -> BackendKind;
    fn call(&self, method: &str, params: Value, timeout: Duration) -> Result<String>;
    fn fork_from_rewind(
        &self,
        _thread_id: &str,
        _request: RewindForkRequest,
        _timeout: Duration,
    ) -> Result<String> {
        bail!("backend does not support rewind fork")
    }
    fn respond(&self, request_id: &Value, result: Value) -> Result<()>;
    fn respond_error(&self, request_id: &Value, code: i64, message: &str) -> Result<()>;
    fn take_events_rx(&mut self) -> Result<mpsc::Receiver<String>>;
    fn stop(&mut self);
}
