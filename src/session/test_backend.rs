//! Record backend calls for terminal-free session command tests.

use std::collections::VecDeque;
use std::sync::{mpsc, Mutex};
use std::time::Duration;

use anyhow::{bail, ensure, Context, Result};
use serde_json::Value;

use crate::backend::{BackendClient, BackendKind, EventReceiver};

/// Record exact native calls and optionally serve fixed read-only replies.
#[derive(Default)]
pub(crate) struct TestBackend {
    pub(crate) acknowledgements: Mutex<Vec<usize>>,
    pub(crate) reads: Mutex<VecDeque<(String, Value)>>,
    pub(crate) calls: Mutex<Vec<(String, Value, Value)>>,
    pub(crate) replies: Mutex<Vec<(Value, Value)>>,
    pub(crate) fail: bool,
}

impl BackendClient for TestBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Codex
    }
    fn call(&self, method: &str, params: Value, _timeout: Duration) -> Result<String> {
        let (expected, response) = self
            .reads
            .lock()
            .expect("reads")
            .pop_front()
            .context("unexpected blocking call")?;
        ensure!(method == expected, "expected {expected}, received {method}");
        self.calls
            .lock()
            .expect("calls")
            .push((method.into(), params, Value::Null));
        Ok(response.to_string())
    }
    fn request_control_with_context(
        &self,
        method: &str,
        params: Value,
        context: Value,
    ) -> Result<()> {
        if self.fail {
            bail!("write failed");
        }
        self.calls
            .lock()
            .expect("calls")
            .push((method.into(), params, context));
        Ok(())
    }
    fn respond(&self, id: &Value, result: Value) -> Result<()> {
        if self.fail {
            bail!("write failed");
        }
        self.replies
            .lock()
            .expect("replies")
            .push((id.clone(), result));
        Ok(())
    }
    fn respond_error(&self, _id: &Value, _code: i64, _message: &str) -> Result<()> {
        bail!("unexpected error reply")
    }
    fn take_events_rx(&mut self) -> Result<EventReceiver> {
        Ok(mpsc::channel().1.into())
    }
    fn acknowledge_event(&self, bytes: usize) {
        self.acknowledgements.lock().expect("acks").push(bytes);
    }
    fn stop(&mut self) {}
}
