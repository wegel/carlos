//! Grok backend tests: CLI, translation, session listing, and turn queueing.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{bail, Result};
use serde_json::{json, Value};

use super::{
    parse_cli_args, submit_dequeued_turn_text, submit_turn_text, submit_turn_text_with_history,
    AppState, Backend, Role,
};
use crate::backend::{BackendClient, BackendKind, RewindForkRequest};
use crate::grok_backend::{
    build_grok_command_for_test, finish_grok_prompt_turn_for_test, grok_model_catalog,
    is_hidden_rewind_point, load_grok_thread_summaries_from, parse_iso_timestamp_secs,
    parse_rewind_points, resolve_rewind_target, translate_grok_notification_for_test,
    GrokRewindPoint, GrokRuntimeSettings, GrokTranslationState,
};

fn collect_lines(state: &mut GrokTranslationState, events: &[(&str, Value)]) -> Vec<String> {
    let mut out = Vec::new();
    for (method, params) in events {
        out.extend(translate_grok_notification_for_test(state, method, params.clone()).lines);
    }
    out
}

fn methods(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .filter_map(|line| {
            serde_json::from_str::<Value>(line).ok().and_then(|value| {
                value
                    .get("method")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
        })
        .collect()
}

fn temp_test_dir(name: &str) -> PathBuf {
    let unique = format!(
        "carlos-{}-{}-{}",
        name,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("unix epoch")
            .as_nanos()
    );
    std::env::temp_dir().join(unique)
}

struct GrokStartMock {
    calls: std::sync::Mutex<Vec<String>>,
}

impl GrokStartMock {
    fn new() -> Self {
        Self {
            calls: std::sync::Mutex::new(Vec::new()),
        }
    }
}

impl BackendClient for GrokStartMock {
    fn kind(&self) -> BackendKind {
        BackendKind::Grok
    }

    fn call(&self, method: &str, _params: Value, _timeout: Duration) -> Result<String> {
        self.calls
            .lock()
            .expect("calls lock")
            .push(method.to_string());
        Ok("{\"jsonrpc\":\"2.0\",\"result\":{}}".to_string())
    }

    fn respond(&self, _request_id: &Value, _result: Value) -> Result<()> {
        bail!("unused in test")
    }

    fn respond_error(&self, _request_id: &Value, _code: i64, _message: &str) -> Result<()> {
        bail!("unused in test")
    }

    fn take_events_rx(&mut self) -> Result<mpsc::Receiver<String>> {
        bail!("unused in test")
    }

    fn stop(&mut self) {}
}

struct GrokRewindMock {
    fork_calls: std::sync::Mutex<Vec<RewindForkRequest>>,
    turn_start_calls: std::sync::Mutex<Vec<String>>,
}

impl GrokRewindMock {
    fn new() -> Self {
        Self {
            fork_calls: std::sync::Mutex::new(Vec::new()),
            turn_start_calls: std::sync::Mutex::new(Vec::new()),
        }
    }
}

impl BackendClient for GrokRewindMock {
    fn kind(&self) -> BackendKind {
        BackendKind::Grok
    }

    fn call(&self, method: &str, params: Value, _timeout: Duration) -> Result<String> {
        assert_eq!(method, "turn/start");
        let text = params["input"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string();
        self.turn_start_calls
            .lock()
            .expect("turn start lock")
            .push(text);
        Ok("{\"jsonrpc\":\"2.0\",\"result\":{}}".to_string())
    }

    fn fork_from_rewind(
        &self,
        _thread_id: &str,
        request: RewindForkRequest,
        _timeout: Duration,
    ) -> Result<String> {
        self.fork_calls.lock().expect("fork lock").push(request);
        Ok(json!({
            "jsonrpc": "2.0",
            "result": { "thread": { "id": "grok-fork" } }
        })
        .to_string())
    }

    fn respond(&self, _request_id: &Value, _result: Value) -> Result<()> {
        bail!("unused in test")
    }

    fn respond_error(&self, _request_id: &Value, _code: i64, _message: &str) -> Result<()> {
        bail!("unused in test")
    }

    fn take_events_rx(&mut self) -> Result<mpsc::Receiver<String>> {
        bail!("unused in test")
    }

    fn stop(&mut self) {}
}

include!("grok_backend_tests/cli_and_translate.rs");
include!("grok_backend_tests/history_and_submit.rs");
