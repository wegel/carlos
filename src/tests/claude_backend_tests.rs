use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use serde_json::Value;

use super::*;
use super::input_events::{submit_turn_text, submit_turn_text_with_history};
use crate::backend::{BackendClient, BackendKind};
use crate::claude_backend::{translate_claude_line, ClaudeTranslationState};

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
    serde_json::from_str::<Value>(line)
        .ok()
        .and_then(|value| value.get("method").and_then(Value::as_str).map(str::to_string))
}

struct ClaudeSteerMock;

impl BackendClient for ClaudeSteerMock {
    fn kind(&self) -> BackendKind {
        BackendKind::Claude
    }

    fn call(&self, _method: &str, _params: Value, _timeout: Duration) -> Result<String> {
        bail!("Claude steer path should queue instead of calling backend")
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

#[test]
fn parse_cli_args_supports_backend_flag() {
    let parsed = parse_cli_args(vec![
        "--backend".to_string(),
        "claude".to_string(),
        "continue".to_string(),
    ])
    .expect("parse");

    assert_eq!(parsed.backend, Backend::Claude);
    assert!(parsed.mode_continue);
}

#[test]
fn parse_cli_args_uses_backend_env_default() {
    let _guard = env_lock().lock().expect("env lock");
    let old = std::env::var_os("CARLOS_BACKEND");
    std::env::set_var("CARLOS_BACKEND", "claude");

    let parsed = parse_cli_args(Vec::<String>::new()).expect("parse");
    assert_eq!(parsed.backend, Backend::Claude);

    match old {
        Some(value) => std::env::set_var("CARLOS_BACKEND", value),
        None => std::env::remove_var("CARLOS_BACKEND"),
    }
}

#[test]
fn submit_turn_text_queues_next_claude_turn_when_turn_active() {
    let client = ClaudeSteerMock;
    let mut app = AppState::new("thread-1".to_string());
    app.active_turn_id = Some("turn-1".to_string());

    submit_turn_text(&client, &mut app, "follow up".to_string());

    let queued = app.dequeue_turn_input(Instant::now()).expect("queued turn");
    assert_eq!(queued.text, "follow up");
    assert!(queued.record_input_history);
    assert_eq!(app.status, "queued for next Claude turn");
}

#[test]
fn submit_turn_text_appends_user_message_for_claude_start() {
    let client = ClaudeStartMock;
    let mut app = AppState::new("thread-1".to_string());

    submit_turn_text(&client, &mut app, "hello".to_string());

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::User);
    assert_eq!(app.messages[0].text, "hello");
}

#[test]
fn submit_turn_text_backfills_existing_history_for_claude_start() {
    let client = ClaudeStartMock;
    let mut app = AppState::new("thread-1".to_string());
    app.push_input_history("hello");

    submit_turn_text(&client, &mut app, "hello".to_string());

    assert_eq!(app.input_history_len(), 1);
    assert_eq!(app.input_history_message_indices(), &[Some(0)]);
}

#[test]
fn submit_turn_text_skips_history_for_internal_claude_turns() {
    let client = ClaudeStartMock;
    let mut app = AppState::new("thread-1".to_string());

    submit_turn_text_with_history(&client, &mut app, "auto".to_string(), false);

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.input_history_len(), 0);
}

#[test]
fn translate_claude_text_turn_emits_codex_style_notifications() {
    let synthetic = collect_synthetic_lines(&[
        r#"{"type":"system","subtype":"init","session_id":"session-1","model":"claude-opus-4-6[1m]"}"#,
        r#"{"type":"stream_event","event":{"type":"message_start","message":{"id":"msg-1"}},"session_id":"session-1"}"#,
        r#"{"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}},"session_id":"session-1"}"#,
        r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"hi"}},"session_id":"session-1"}"#,
        r#"{"type":"stream_event","event":{"type":"content_block_stop","index":0},"session_id":"session-1"}"#,
        r#"{"type":"stream_event","event":{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"input_tokens":3,"cache_creation_input_tokens":544,"cache_read_input_tokens":8762,"output_tokens":4}},"session_id":"session-1"}"#,
        r#"{"type":"result","session_id":"session-1","terminal_reason":"completed","usage":{"input_tokens":3,"cache_creation_input_tokens":544,"cache_read_input_tokens":8762,"output_tokens":4}}"#,
    ]);

    let methods: Vec<String> = synthetic.iter().filter_map(|line| parse_method(line)).collect();
    assert!(methods.contains(&"thread/initialized".to_string()));
    assert!(methods.contains(&"turn/started".to_string()));
    assert!(methods.contains(&"item/started".to_string()));
    assert!(methods.contains(&"item/agentMessage/delta".to_string()));
    assert!(methods.contains(&"item/completed".to_string()));
    assert!(methods.contains(&"thread/tokenUsage/updated".to_string()));
    assert!(methods.contains(&"turn/completed".to_string()));
}

#[test]
fn translate_claude_bash_tool_result_emits_tool_call_and_tool_output_rows() {
    let synthetic = collect_synthetic_lines(&[
        r#"{"type":"stream_event","event":{"type":"message_start","message":{"id":"msg-1"}},"session_id":"session-1"}"#,
        r#"{"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"toolu_1","name":"Bash","input":{},"caller":{"type":"direct"}}},"session_id":"session-1"}"#,
        r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"command\":\"pwd\",\"description\":\"Print working directory\"}"}},"session_id":"session-1"}"#,
        r#"{"type":"stream_event","event":{"type":"content_block_stop","index":0},"session_id":"session-1"}"#,
        r#"{"type":"user","message":{"role":"user","content":[{"tool_use_id":"toolu_1","type":"tool_result","content":"/repo","is_error":false}]},"tool_use_result":{"stdout":"/repo","stderr":"","interrupted":false},"session_id":"session-1"}"#,
    ]);

    let completed: Vec<Value> = synthetic
        .iter()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|value| value.get("method").and_then(Value::as_str) == Some("item/completed"))
        .collect();

    assert_eq!(completed.len(), 2);
    assert_eq!(
        completed[0]["params"]["item"]["type"].as_str(),
        Some("toolCall")
    );
    assert_eq!(
        completed[1]["params"]["item"]["type"].as_str(),
        Some("toolResult")
    );
    assert!(
        completed[1]["params"]["item"]["output"]
            .as_str()
            .unwrap_or("")
            .contains("$ pwd")
    );
}
