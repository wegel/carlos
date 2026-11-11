//! Delegate native operations through a short-lived connection handle, never a locked RPC.

use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::Value;

use super::CodexClient;
use crate::backend::{BackendClient, BackendKind, RewindForkRequest};
use crate::codex_snapshot::CodexSnapshot;

impl BackendClient for CodexClient {
    fn kind(&self) -> BackendKind {
        BackendKind::Codex
    }
    fn call(&self, method: &str, params: Value, timeout: Duration) -> Result<String> {
        self.connection()?.call(method, params, timeout)
    }
    fn notify(&self, method: &str, params: Value) -> Result<()> {
        self.connection()?.notify(method, params)
    }
    fn validate_form(
        &self,
        id: Value,
        schema: Value,
        answer: Option<Value>,
        revision: u64,
    ) -> Result<()> {
        self.connection()?
            .validate_form(id, schema, answer, revision)
    }
    fn request_rate_limits(&self) -> Result<Option<String>> {
        self.connection()?.request_rate_limits()
    }
    fn request_native_control(&self, method: &str, params: Value) -> Result<()> {
        self.connection()?.request_native_control(method, params)
    }
    fn request_control_with_context(
        &self,
        method: &str,
        params: Value,
        context: Value,
    ) -> Result<()> {
        self.connection()?
            .request_control_with_context(method, params, context)
    }
    fn acknowledge_event(&self, length: usize) {
        self.shared.events.handled(length);
    }
    fn fork_from_rewind(
        &self,
        thread: &str,
        request: RewindForkRequest,
        timeout: Duration,
    ) -> Result<String> {
        self.connection()?
            .fork_from_rewind(thread, request, timeout)
    }
    fn respond(&self, id: &Value, result: Value) -> Result<()> {
        self.connection()?.respond(id, result)
    }
    fn respond_error(&self, id: &Value, code: i64, message: &str) -> Result<()> {
        self.connection()?.respond_error(id, code, message)
    }
    fn take_events_rx(&mut self) -> Result<mpsc::Receiver<String>> {
        self.receiver
            .take()
            .context("Codex events receiver already taken")
    }
    fn abort_connection(&self) {
        if let Ok(client) = self.connection() {
            client.stop();
        }
    }
    fn reconnect(&self, thread: &str) -> Result<u64> {
        CodexClient::reconnect(self, thread.into())
    }
    fn take_reconnect_snapshot(&self, generation: u64) -> Option<CodexSnapshot> {
        let mut slot = self.shared.snapshot.lock().ok()?;
        if slot.as_ref()?.0 != generation {
            return None;
        }
        slot.take().map(|(_, snapshot)| snapshot)
    }
    fn connection_generation(&self) -> Option<u64> {
        Some(self.shared.generation.load(Ordering::SeqCst))
    }
    fn stop(&mut self) {
        CodexClient::stop(self);
    }
}
