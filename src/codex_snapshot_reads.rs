//! Read bounded native reconnect facts under one original deadline.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use anyhow::{bail, ensure, Context, Result};
use serde_json::{json, Value};

use crate::backend::{BackendClient, BackendKind};
use crate::protocol::AppServerClient;
use crate::protocol_history::is_unsupported_method;
use crate::protocol_params::extract_result_object;

/// Keep blocking public reads on the reconnect worker, away from the UI.
pub(crate) struct SnapshotReader<'a> {
    client: &'a AppServerClient,
    pub(crate) deadline: Instant,
}

impl<'a> SnapshotReader<'a> {
    /// Give the entire rebuild one deadline, including every public page.
    pub(crate) fn new(client: &'a AppServerClient, deadline: Instant) -> Self {
        Self { client, deadline }
    }

    /// Read a result, accepting only actual method absence as an optional capability.
    pub(crate) fn optional(&self, method: &str, params: Value) -> Result<Option<Value>> {
        let line = self.call(method, params, Duration::from_secs(5))?;
        let parsed: Value = serde_json::from_str(&line)?;
        if parsed.get("error").is_some_and(is_unsupported_method)
            || method == "thread/goal/get"
                && parsed["error"]["message"] == "goals feature is disabled"
        {
            return Ok(None);
        }
        Ok(Some(extract_result_object(&line)?["result"].clone()))
    }

    /// Keep every advertised model page under the same deadline and eight MiB cap.
    pub(crate) fn catalog(&self) -> Result<Value> {
        let mut data = Vec::new();
        let mut cursor = Value::Null;
        let mut seen = HashSet::new();
        let mut bytes = 0;
        loop {
            let Some(page) = self.optional("model/list", json!({"cursor":cursor,"limit":100}))?
            else {
                return Ok(json!({"data":[]}));
            };
            bytes += page.to_string().len();
            ensure!(
                bytes <= 8 * 1024 * 1024 && data.len() < 4096,
                "Model catalog exceeds8 MiB or4096 entries"
            );
            data.extend(
                page["data"]
                    .as_array()
                    .context("Malformed model catalog")?
                    .iter()
                    .cloned(),
            );
            ensure!(data.len() <= 4096, "Model catalog exceeds4096 entries");
            cursor = page["nextCursor"].clone();
            if cursor.is_null() {
                break;
            }
            ensure!(
                cursor.is_string() && seen.insert(cursor.to_string()),
                "Invalid/repeated model cursor"
            );
        }
        Ok(json!({"data":data}))
    }

    /// Assemble one native queue without combining large entries in a response frame.
    pub(crate) fn queue(&self, thread: &str) -> Result<Option<Value>> {
        let mut entries = Vec::new();
        let mut cursors = HashSet::new();
        let mut cursor = Value::Null;
        let mut bytes = 0;
        loop {
            let Some(page) = self.optional(
                "thread/queue/list",
                json!({
                "threadId":thread,"cursor":cursor,"limit":1}),
            )?
            else {
                return Ok(None);
            };
            bytes += page.to_string().len();
            ensure!(
                bytes <= 16 * 1024 * 1024 && entries.len() < 4096,
                "Codex reconnect queue exceeds 16 MiB or 4096 entries"
            );
            let rows = page["data"]
                .as_array()
                .context("Malformed native queue page")?;
            for row in rows {
                ensure!(
                    row["id"].is_string()
                        && !entries.iter().any(|old: &Value| old["id"] == row["id"]),
                    "Missing or repeated native queued ID"
                );
                entries.push(row.clone());
                ensure!(entries.len() <= 4096, "Queue snapshot exceeds4096 entries");
            }
            cursor = page["nextCursor"].clone();
            if cursor.is_null() {
                break;
            }
            ensure!(
                cursor.is_string() && cursors.insert(cursor.to_string()),
                "Invalid/repeated queue cursor"
            );
        }
        Ok(Some(json!(entries)))
    }
}

impl BackendClient for SnapshotReader<'_> {
    fn kind(&self) -> BackendKind {
        BackendKind::Codex
    }
    fn call(&self, method: &str, params: Value, timeout: Duration) -> Result<String> {
        let remaining = self
            .deadline
            .checked_duration_since(Instant::now())
            .context("Codex reconnect read exceeded20 seconds")?;
        let line = self
            .client
            .call(method, params.clone(), timeout.min(remaining))?;
        Ok(line)
    }
    fn notify(&self, method: &str, params: Value) -> Result<()> {
        self.client.notify(method, params)
    }
    fn respond(&self, _: &Value, _: Value) -> Result<()> {
        bail!("Snapshot reader cannot answer callbacks")
    }
    fn respond_error(&self, _: &Value, _: i64, _: &str) -> Result<()> {
        bail!("Snapshot reader cannot answer callbacks")
    }
    fn take_events_rx(&mut self) -> Result<std::sync::mpsc::Receiver<String>> {
        bail!("Snapshot reader has no event stream")
    }
    fn stop(&mut self) {}
}
