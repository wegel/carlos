//! Accept form-check jobs so tests can deliver their replies explicitly.

use std::sync::Mutex;
use std::time::Duration;

use anyhow::Result;
use carlos_core::backend::{BackendClient, BackendKind, EventReceiver};
use serde_json::{json, Value};

use super::super::codex_question_support::Peer;

/// Record answers while tests decide when a form-check reply arrives.
#[derive(Default)]
pub(super) struct ValidationPeer {
    pub(super) peer: Peer,
    pub(super) checks: Mutex<Vec<Value>>,
}

impl BackendClient for ValidationPeer {
    fn kind(&self) -> BackendKind {
        BackendKind::Codex
    }
    fn call(&self, method: &str, params: Value, timeout: Duration) -> Result<String> {
        self.peer.call(method, params, timeout)
    }
    fn respond(&self, id: &Value, answer: Value) -> Result<()> {
        self.peer.respond(id, answer)
    }
    fn respond_error(&self, id: &Value, code: i64, message: &str) -> Result<()> {
        self.peer.respond_error(id, code, message)
    }
    fn validate_form(
        &self,
        id: Value,
        schema: Value,
        answer: Option<Value>,
        revision: u64,
    ) -> Result<()> {
        self.checks
            .lock()
            .expect("form checks")
            .push(json!([id, schema, answer, revision]));
        Ok(())
    }
    fn take_events_rx(&mut self) -> Result<EventReceiver> {
        anyhow::bail!("No receiver in this fixture")
    }
    fn stop(&mut self) {}
}
