//! Record only nonblocking native quota requests and reject synchronous calls.

use std::cell::Cell;
use std::time::Duration;

use anyhow::{bail, Result};
use serde_json::{json, Value};

use crate::backend::{BackendClient, BackendKind, EventReceiver};
use crate::session::usage::WEEK_MINUTES;

pub(super) const NOW: i64 = 1_800_000_000;

pub(super) fn window(used: u64, days: f64) -> Value {
    json!({"usedPercent":used,"windowDurationMins":WEEK_MINUTES,
        "resetsAt":NOW+(days*86_400.0) as i64})
}

pub(super) struct QuotaClient {
    pub(super) reads: Cell<u32>,
}

impl BackendClient for QuotaClient {
    fn kind(&self) -> BackendKind {
        BackendKind::Codex
    }
    fn call(&self, _: &str, _: Value, _: Duration) -> Result<String> {
        panic!("quota reads must not wait on synchronous calls")
    }
    fn request_rate_limits(&self) -> Result<Option<String>> {
        self.reads.set(self.reads.get() + 1);
        Ok(Some(format!("read-{}", self.reads.get())))
    }
    fn respond(&self, _: &Value, _: Value) -> Result<()> {
        bail!("unused")
    }
    fn respond_error(&self, _: &Value, _: i64, _: &str) -> Result<()> {
        bail!("unused")
    }
    fn take_events_rx(&mut self) -> Result<EventReceiver> {
        bail!("unused")
    }
    fn stop(&mut self) {}
}
