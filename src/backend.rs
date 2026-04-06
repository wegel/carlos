use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BackendKind {
    Codex,
    Claude,
}

pub(crate) trait BackendClient {
    fn kind(&self) -> BackendKind;
    fn call(&self, method: &str, params: Value, timeout: Duration) -> Result<String>;
    fn respond(&self, request_id: &Value, result: Value) -> Result<()>;
    fn respond_error(&self, request_id: &Value, code: i64, message: &str) -> Result<()>;
    fn take_events_rx(&mut self) -> Result<mpsc::Receiver<String>>;
    fn stop(&mut self);
}
