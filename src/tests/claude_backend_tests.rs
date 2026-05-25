use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use serde_json::Value;

use super::{
    handle_notification_line, parse_cli_args, submit_turn_text, submit_turn_text_with_history,
    AppState, Backend, Role,
};
use crate::backend::{BackendClient, BackendKind};
use crate::claude_backend::{
    build_claude_command_for_test, claude_approval_follow_up_text, claude_model_catalog,
    claude_project_dir_name, claude_recovery_launch_mode, collect_live_forwarded_lines_for_test,
    fork_claude_local_history_from_projects_root, load_claude_local_history,
    load_claude_local_history_from_projects_root, probe_claude_startup_for_test,
    translate_claude_line, ClaudeLaunchMode, ClaudeTranslationState,
};

fn collect_synthetic_lines(lines: &[&str]) -> Vec<String> {
    let mut state = ClaudeTranslationState::default();
    let mut out = Vec::new();
    for line in lines {
        let translated = translate_claude_line(&mut state, line).expect("translate");
        out.extend(translated.lines);
    }
    out
}

fn parse_method(line: &str) -> Option<String> {
    serde_json::from_str::<Value>(line).ok().and_then(|value| {
        value
            .get("method")
            .and_then(Value::as_str)
            .map(str::to_string)
    })
}

fn command_args(command: &std::process::Command) -> Vec<String> {
    command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
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

fn write_session_file(root: &Path, cwd: &Path, session_id: &str, lines: &[&str]) -> PathBuf {
    let project_dir = root.join(claude_project_dir_name(cwd));
    fs::create_dir_all(&project_dir).expect("create project dir");
    let path = project_dir.join(format!("{session_id}.jsonl"));
    let body = format!("{}\n", lines.join("\n"));
    fs::write(&path, body).expect("write session file");
    path
}

struct ClaudeSteerMock {
    calls: std::sync::Mutex<Vec<String>>,
}

impl ClaudeSteerMock {
    fn new() -> Self {
        Self {
            calls: std::sync::Mutex::new(Vec::new()),
        }
    }
}

impl BackendClient for ClaudeSteerMock {
    fn kind(&self) -> BackendKind {
        BackendKind::Claude
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

struct ClaudeStartMock;

impl BackendClient for ClaudeStartMock {
    fn kind(&self) -> BackendKind {
        BackendKind::Claude
    }

    fn call(&self, method: &str, _params: Value, _timeout: Duration) -> Result<String> {
        assert_eq!(method, "turn/start");
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

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

include!("claude_backend_tests/part_1.rs");

include!("claude_backend_tests/part_2.rs");

include!("claude_backend_tests/part_3.rs");

include!("claude_backend_tests/part_4.rs");
