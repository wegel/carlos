use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use serde_json::Value;

use super::turn_submit::{submit_turn_text, submit_turn_text_with_history};
use super::*;
use crate::backend::{BackendClient, BackendKind};
use crate::claude_backend::{
    build_claude_command_for_test, claude_approval_follow_up_text, claude_project_dir_name,
    claude_recovery_launch_mode, load_claude_local_history,
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
fn build_claude_command_disallows_plan_mode_by_default() {
    let _guard = env_lock().lock().expect("env lock");
    let old = std::env::var_os("CARLOS_CLAUDE_ALLOW_PLAN_MODE");
    std::env::remove_var("CARLOS_CLAUDE_ALLOW_PLAN_MODE");

    let command = build_claude_command_for_test(
        Path::new("/repo"),
        &ClaudeLaunchMode::Continue,
    );
    let args = command_args(&command);
    assert!(args.windows(3).any(|window| {
        window[0] == "--disallowedTools"
            && window[1] == "EnterPlanMode"
            && window[2] == "Agent(Plan)"
    }));

    match old {
        Some(value) => std::env::set_var("CARLOS_CLAUDE_ALLOW_PLAN_MODE", value),
        None => std::env::remove_var("CARLOS_CLAUDE_ALLOW_PLAN_MODE"),
    }
}

#[test]
fn build_claude_command_allows_plan_mode_when_opted_in() {
    let _guard = env_lock().lock().expect("env lock");
    let old = std::env::var_os("CARLOS_CLAUDE_ALLOW_PLAN_MODE");
    std::env::set_var("CARLOS_CLAUDE_ALLOW_PLAN_MODE", "1");

    let command = build_claude_command_for_test(
        Path::new("/repo"),
        &ClaudeLaunchMode::Continue,
    );
    let args = command_args(&command);
    assert!(!args.iter().any(|arg| arg == "--disallowedTools"));

    match old {
        Some(value) => std::env::set_var("CARLOS_CLAUDE_ALLOW_PLAN_MODE", value),
        None => std::env::remove_var("CARLOS_CLAUDE_ALLOW_PLAN_MODE"),
    }
}

#[test]
fn probe_claude_startup_reports_early_stderr_exit() {
    let mut command = std::process::Command::new("sh");
    command.args(["-c", "printf 'startup failed\\n' >&2; exit 1"]);

    let err = probe_claude_startup_for_test(&mut command).expect_err("startup should fail");
    assert!(err.to_string().contains("startup failed"));
}

#[test]
fn probe_claude_startup_accepts_first_stdout_line() {
    let mut command = std::process::Command::new("sh");
    command.args(["-c", "printf 'ready\\n'; sleep 1"]);

    probe_claude_startup_for_test(&mut command).expect("startup should succeed");
}

#[test]
fn load_claude_local_history_uses_tilde_claude_config_dir() {
    let _guard = env_lock().lock().expect("env lock");
    let old_home = std::env::var_os("HOME");
    let old_config_dir = std::env::var_os("CLAUDE_CONFIG_DIR");
    let root = temp_test_dir("claude-config-dir-tilde");
    let home = root.join("home");
    let config_dir = home.join(".claude-tt");
    let projects_root = config_dir.join("projects");
    let cwd = Path::new("/repo");
    let session_id = "session-tilde-config";
    let _ = fs::create_dir_all(&projects_root);
    write_session_file(
        &projects_root,
        cwd,
        session_id,
        &[r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Hello from custom config"}]},"sessionId":"session-tilde-config"}"#],
    );

    std::env::set_var("HOME", &home);
    std::env::set_var("CLAUDE_CONFIG_DIR", "~/.claude-tt");

    let imported = load_claude_local_history(
        cwd,
        &ClaudeLaunchMode::Resume(session_id.to_string()),
    )
    .expect("history")
    .expect("imported history");
    assert_eq!(imported.session_id, session_id);
    assert_eq!(imported.imported_item_count, 1);

    match old_home {
        Some(value) => std::env::set_var("HOME", value),
        None => std::env::remove_var("HOME"),
    }
    match old_config_dir {
        Some(value) => std::env::set_var("CLAUDE_CONFIG_DIR", value),
        None => std::env::remove_var("CLAUDE_CONFIG_DIR"),
    }
}

#[test]
fn translate_claude_hides_plan_write_tools_under_tilde_config_dir() {
    let _guard = env_lock().lock().expect("env lock");
    let old_home = std::env::var_os("HOME");
    let old_config_dir = std::env::var_os("CLAUDE_CONFIG_DIR");
    let root = temp_test_dir("claude-config-dir-plan-write");
    let home = root.join("home");
    let plan_path = home.join(".claude-tt").join("plans").join("test-plan.md");
    let plan_path_json =
        serde_json::to_string(plan_path.to_str().expect("utf-8 plan path")).expect("json path");
    let delta_line = serde_json::json!({
        "type": "stream_event",
        "event": {
            "type": "content_block_delta",
            "index": 0,
            "delta": {
                "type": "input_json_delta",
                "partial_json": format!("{{\"file_path\":{},\"content\":\"# Plan\"}}", plan_path_json),
            }
        },
        "session_id": "session-1",
    })
    .to_string();
    let _ = fs::create_dir_all(home.join(".claude-tt").join("plans"));
    std::env::set_var("HOME", &home);
    std::env::set_var("CLAUDE_CONFIG_DIR", "~/.claude-tt");

    let synthetic = collect_synthetic_lines(&[
        r#"{"type":"stream_event","event":{"type":"message_start","message":{"id":"msg-1"}},"session_id":"session-1"}"#,
        r#"{"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"toolu_write","name":"Write","input":{},"caller":{"type":"direct"}}},"session_id":"session-1"}"#,
        &delta_line,
        r#"{"type":"stream_event","event":{"type":"content_block_stop","index":0},"session_id":"session-1"}"#,
        r#"{"type":"user","message":{"role":"user","content":[{"tool_use_id":"toolu_write","type":"tool_result","content":"File created successfully","is_error":false}]},"session_id":"session-1"}"#,
    ]);

    let completed: Vec<Value> = synthetic
        .iter()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|value| value.get("method").and_then(Value::as_str) == Some("item/completed"))
        .collect();
    assert!(completed.is_empty());

    match old_home {
        Some(value) => std::env::set_var("HOME", value),
        None => std::env::remove_var("HOME"),
    }
    match old_config_dir {
        Some(value) => std::env::set_var("CLAUDE_CONFIG_DIR", value),
        None => std::env::remove_var("CLAUDE_CONFIG_DIR"),
    }
}

#[test]
fn submit_turn_text_sends_claude_steer_when_turn_active() {
    let client = ClaudeSteerMock::new();
    let mut app = AppState::new("thread-1".to_string());
    app.active_turn_id = Some("turn-1".to_string());

    submit_turn_text(&client, &mut app, "follow up".to_string());

    assert_eq!(
        client.calls.lock().expect("calls lock").as_slice(),
        ["turn/steer"]
    );
    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::User);
    assert_eq!(app.messages[0].text, "follow up");
    assert_eq!(app.input_history_len(), 1);
    assert_eq!(app.status, "sent steer");
}

#[test]
fn submit_turn_text_queues_behind_existing_claude_turns() {
    let client = ClaudeSteerMock::new();
    let mut app = AppState::new("thread-1".to_string());
    app.queue_turn_input("first");

    submit_turn_text(&client, &mut app, "second".to_string());

    let first = app
        .dequeue_turn_input(Instant::now())
        .expect("first queued turn");
    let second = app
        .dequeue_turn_input(Instant::now())
        .expect("second queued turn");
    assert_eq!(first.text, "first");
    assert_eq!(second.text, "second");
    assert_eq!(app.status, "queued behind pending Claude turn");
}

#[test]
fn submit_turn_text_does_not_queue_behind_delayed_ralph_continuation() {
    let client = ClaudeStartMock;
    let mut app = AppState::new("thread-1".to_string());
    app.queue_ralph_continuation("continue");

    submit_turn_text(&client, &mut app, "user prompt".to_string());

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].text, "user prompt");
    assert_eq!(app.status, "sent turn");
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
fn submit_turn_text_marks_claude_turn_active_immediately() {
    let client = ClaudeStartMock;
    let mut app = AppState::new("thread-1".to_string());

    submit_turn_text(&client, &mut app, "hello".to_string());

    assert_eq!(app.active_turn_id.as_deref(), Some("claude-turn-pending"));
    assert_eq!(app.turn_start_message_idx, Some(1));
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

    let methods: Vec<String> = synthetic
        .iter()
        .filter_map(|line| parse_method(line))
        .collect();
    assert!(methods.contains(&"thread/initialized".to_string()));
    assert!(methods.contains(&"turn/started".to_string()));
    assert!(methods.contains(&"item/started".to_string()));
    assert!(methods.contains(&"item/agentMessage/delta".to_string()));
    assert!(methods.contains(&"item/completed".to_string()));
    assert!(methods.contains(&"thread/tokenUsage/updated".to_string()));
    assert!(methods.contains(&"turn/completed".to_string()));
}

#[test]
fn translate_claude_assistant_snapshot_backfills_missing_text_blocks() {
    let synthetic = collect_synthetic_lines(&[
        r#"{"type":"system","subtype":"init","session_id":"session-1","model":"claude-opus-4-6"}"#,
        r#"{"type":"assistant","session_id":"session-1","message":{"id":"msg-1","role":"assistant","model":"claude-opus-4-6","content":[{"type":"text","text":"OK"}]}}"#,
        r#"{"type":"result","session_id":"session-1","terminal_reason":"completed","usage":{"input_tokens":3,"cache_creation_input_tokens":544,"cache_read_input_tokens":8762,"output_tokens":4}}"#,
    ]);

    let methods: Vec<String> = synthetic
        .iter()
        .filter_map(|line| parse_method(line))
        .collect();
    assert!(methods.contains(&"turn/started".to_string()));
    assert!(methods.contains(&"item/completed".to_string()));
    assert!(methods.contains(&"turn/completed".to_string()));

    let completed: Vec<Value> = synthetic
        .iter()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|value| value.get("method").and_then(Value::as_str) == Some("item/completed"))
        .collect();
    assert_eq!(completed.len(), 1);
    assert_eq!(
        completed[0]["params"]["item"]["text"].as_str(),
        Some("OK")
    );
}

#[test]
fn translate_claude_assistant_snapshot_does_not_duplicate_streamed_text() {
    let synthetic = collect_synthetic_lines(&[
        r#"{"type":"stream_event","event":{"type":"message_start","message":{"id":"msg-1"}},"session_id":"session-1"}"#,
        r#"{"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}},"session_id":"session-1"}"#,
        r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"OK"}},"session_id":"session-1"}"#,
        r#"{"type":"assistant","session_id":"session-1","message":{"id":"msg-1","role":"assistant","model":"claude-opus-4-6","content":[{"type":"text","text":"OK"}]}}"#,
        r#"{"type":"stream_event","event":{"type":"content_block_stop","index":0},"session_id":"session-1"}"#,
        r#"{"type":"result","session_id":"session-1","terminal_reason":"completed","usage":{"input_tokens":3,"cache_creation_input_tokens":544,"cache_read_input_tokens":8762,"output_tokens":4}}"#,
    ]);

    let completed: Vec<Value> = synthetic
        .iter()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|value| value.get("method").and_then(Value::as_str) == Some("item/completed"))
        .collect();
    assert_eq!(completed.len(), 1);
    assert_eq!(
        completed[0]["params"]["item"]["text"].as_str(),
        Some("OK")
    );
}

#[test]
fn translate_claude_assistant_snapshot_emits_follow_up_message_after_tool_result() {
    let synthetic = collect_synthetic_lines(&[
        r#"{"type":"stream_event","event":{"type":"message_start","message":{"id":"msg-1"}},"session_id":"session-1"}"#,
        r#"{"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"toolu_edit","name":"Edit","input":{},"caller":{"type":"direct"}}},"session_id":"session-1"}"#,
        r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"file_path\":\"/repo/file.swift\",\"replace_all\":false}"}},"session_id":"session-1"}"#,
        r#"{"type":"stream_event","event":{"type":"content_block_stop","index":0},"session_id":"session-1"}"#,
        r#"{"type":"user","message":{"role":"user","content":[{"tool_use_id":"toolu_edit","type":"tool_result","content":"updated","is_error":false}]},"session_id":"session-1"}"#,
        r#"{"type":"assistant","session_id":"session-1","message":{"id":"msg-2","role":"assistant","model":"claude-opus-4-6","content":[{"type":"text","text":"Done. Here's the summary."}]}}"#,
    ]);

    let completed: Vec<Value> = synthetic
        .iter()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|value| value.get("method").and_then(Value::as_str) == Some("item/completed"))
        .collect();

    assert!(completed.iter().any(|value| {
        value["params"]["item"]["type"].as_str() == Some("agentMessage")
            && value["params"]["item"]["text"].as_str() == Some("Done. Here's the summary.")
    }));
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
    assert!(completed[1]["params"]["item"]["output"]
        .as_str()
        .unwrap_or("")
        .contains("$ pwd"));
}

#[test]
fn translate_claude_exit_plan_fallback_emits_pending_approval_request() {
    let synthetic = collect_synthetic_lines(&[
        r#"{"type":"stream_event","event":{"type":"message_start","message":{"id":"msg-1"}},"session_id":"session-1"}"#,
        r#"{"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"toolu_exit","name":"ExitPlanMode","input":{},"caller":{"type":"direct"}}},"session_id":"session-1"}"#,
        r##"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"plan\":\"# Plan\\nShip it\",\"planFilePath\":\"/tmp/plan.md\",\"allowedPrompts\":[{\"prompt\":\"run cargo test\",\"tool\":\"Bash\"}]}"}},"session_id":"session-1"}"##,
        r#"{"type":"stream_event","event":{"type":"content_block_stop","index":0},"session_id":"session-1"}"#,
        r#"{"type":"user","message":{"role":"user","content":[{"tool_use_id":"toolu_exit","type":"tool_result","content":"Exit plan mode?","is_error":true}]},"session_id":"session-1"}"#,
    ]);

    let request = synthetic
        .iter()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|value| {
            value.get("method").and_then(Value::as_str) == Some("claude/exitPlan/requestApproval")
        })
        .expect("approval request");

    assert_eq!(request["id"]["backend"].as_str(), Some("claude"));
    assert_eq!(request["id"]["kind"].as_str(), Some("exitPlanMode"));
    assert_eq!(request["params"]["toolUseId"].as_str(), Some("toolu_exit"));
    assert_eq!(
        request["params"]["planFilePath"].as_str(),
        Some("/tmp/plan.md")
    );
    assert_eq!(
        request["params"]["allowedPrompts"][0]["prompt"].as_str(),
        Some("run cargo test")
    );

    let completed: Vec<Value> = synthetic
        .iter()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|value| value.get("method").and_then(Value::as_str) == Some("item/completed"))
        .collect();
    assert!(completed.is_empty());
}

#[test]
fn translate_claude_hides_internal_agent_and_plan_write_tools() {
    let synthetic = collect_synthetic_lines(&[
        r#"{"type":"stream_event","event":{"type":"message_start","message":{"id":"msg-1"}},"session_id":"session-1"}"#,
        r#"{"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"toolu_agent","name":"Agent","input":{},"caller":{"type":"direct"}}},"session_id":"session-1"}"#,
        r##"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"subagent_type\":\"Plan\",\"description\":\"Draft plan\",\"prompt\":\"Do the plan\"}"}},"session_id":"session-1"}"##,
        r#"{"type":"stream_event","event":{"type":"content_block_stop","index":0},"session_id":"session-1"}"#,
        r#"{"type":"user","message":{"role":"user","content":[{"tool_use_id":"toolu_agent","type":"tool_result","content":"done","is_error":false}]},"session_id":"session-1"}"#,
        r#"{"type":"stream_event","event":{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_write","name":"Write","input":{},"caller":{"type":"direct"}}},"session_id":"session-1"}"#,
        r##"{"type":"stream_event","event":{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"file_path\":\"/home/wegel/.claude/plans/test-plan.md\",\"content\":\"# Plan\"}"}},"session_id":"session-1"}"##,
        r#"{"type":"stream_event","event":{"type":"content_block_stop","index":1},"session_id":"session-1"}"#,
        r#"{"type":"user","message":{"role":"user","content":[{"tool_use_id":"toolu_write","type":"tool_result","content":"File created successfully","is_error":false}]},"session_id":"session-1"}"#,
    ]);

    let completed: Vec<Value> = synthetic
        .iter()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|value| value.get("method").and_then(Value::as_str) == Some("item/completed"))
        .collect();
    assert!(completed.is_empty());
}

#[test]
fn claude_exit_plan_follow_up_text_matches_accept_and_decline() {
    let request_id = serde_json::json!({
        "backend": "claude",
        "kind": "exitPlanMode",
        "toolUseId": "toolu_exit",
    });

    assert_eq!(
        claude_approval_follow_up_text(&request_id, &serde_json::json!({"decision":"accept"}))
            .expect("accept"),
        Some("The plan is approved. Continue with the planned implementation now.")
    );
    assert_eq!(
        claude_approval_follow_up_text(&request_id, &serde_json::json!({"decision":"decline"}))
            .expect("decline"),
        Some(
            "Do not exit plan mode yet. Stay in plan mode, revise the plan, and then present an updated plan for approval."
        )
    );
}

#[test]
fn claude_project_dir_name_encodes_current_worktree_path() {
    assert_eq!(
        claude_project_dir_name(Path::new("/var/home/wegel/work/perso/stormvault")),
        "-var-home-wegel-work-perso-stormvault"
    );
}

#[test]
fn claude_recovery_launch_mode_prefers_live_session_id() {
    let recovered = claude_recovery_launch_mode(&ClaudeLaunchMode::Continue, Some("session-123"));

    match recovered {
        ClaudeLaunchMode::Resume(session_id) => assert_eq!(session_id, "session-123"),
        other => panic!("expected resume launch mode, got {other:?}"),
    }
}

#[test]
fn claude_recovery_launch_mode_falls_back_without_live_session_id() {
    let recovered =
        claude_recovery_launch_mode(&ClaudeLaunchMode::Resume("session-123".to_string()), None);

    match recovered {
        ClaudeLaunchMode::Resume(session_id) => assert_eq!(session_id, "session-123"),
        other => panic!("expected resume launch mode, got {other:?}"),
    }
}

#[test]
fn local_claude_history_keeps_pending_exit_plan_when_later_tool_result_shares_record() {
    let root = temp_test_dir("claude-history-exit-plan-shared-record");
    let cwd = Path::new("/repo");
    write_session_file(
        &root,
        cwd,
        "session-exit-plan-shared-record",
        &[
            r##"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_exit","name":"ExitPlanMode","input":{"plan":"# Plan\nShip it"}},{"type":"tool_use","id":"toolu_bash","name":"Bash","input":{"command":"pwd","description":"Print working directory"}}]},"sessionId":"session-exit-plan-shared-record"}"##,
            r#"{"type":"user","message":{"role":"user","content":[{"tool_use_id":"toolu_exit","type":"tool_result","content":"Exit plan mode?","is_error":true},{"tool_use_id":"toolu_bash","type":"tool_result","content":"/repo","is_error":false}]},"toolUseResult":{"stdout":"/repo","stderr":"","interrupted":false},"sessionId":"session-exit-plan-shared-record"}"#,
        ],
    );

    let imported = load_claude_local_history_from_projects_root(
        &root,
        cwd,
        &ClaudeLaunchMode::Resume("session-exit-plan-shared-record".to_string()),
    )
    .expect("import")
    .expect("history");

    let pending = imported
        .pending_approval_request
        .as_deref()
        .expect("pending approval request");
    let parsed: Value = serde_json::from_str(pending).expect("parse request");
    assert_eq!(parsed["params"]["toolUseId"].as_str(), Some("toolu_exit"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn local_claude_history_ignores_malformed_tool_result_after_pending_exit_plan() {
    let root = temp_test_dir("claude-history-exit-plan-malformed-followup");
    let cwd = Path::new("/repo");
    write_session_file(
        &root,
        cwd,
        "session-exit-plan-malformed-followup",
        &[
            r##"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_exit","name":"ExitPlanMode","input":{"plan":"# Plan\nShip it"}}]},"sessionId":"session-exit-plan-malformed-followup"}"##,
            r#"{"type":"user","message":{"role":"user","content":[{"tool_use_id":"toolu_exit","type":"tool_result","content":"Exit plan mode?","is_error":true}]},"sessionId":"session-exit-plan-malformed-followup"}"#,
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","content":"ignored malformed fragment","is_error":false}]},"sessionId":"session-exit-plan-malformed-followup"}"#,
        ],
    );

    let imported = load_claude_local_history_from_projects_root(
        &root,
        cwd,
        &ClaudeLaunchMode::Resume("session-exit-plan-malformed-followup".to_string()),
    )
    .expect("import")
    .expect("history");

    let pending = imported
        .pending_approval_request
        .as_deref()
        .expect("pending approval request");
    let parsed: Value = serde_json::from_str(pending).expect("parse request");
    assert_eq!(parsed["params"]["toolUseId"].as_str(), Some("toolu_exit"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn local_claude_history_recovers_pending_exit_plan_approval() {
    let root = temp_test_dir("claude-history-exit-plan");
    let cwd = Path::new("/repo");
    write_session_file(
        &root,
        cwd,
        "session-exit-plan",
        &[
            r##"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_exit","name":"ExitPlanMode","input":{"plan":"# Plan\nShip it","planFilePath":"/tmp/plan.md","allowedPrompts":[{"prompt":"run cargo test","tool":"Bash"}]}}]},"sessionId":"session-exit-plan"}"##,
            r#"{"type":"user","message":{"role":"user","content":[{"tool_use_id":"toolu_exit","type":"tool_result","content":"Exit plan mode?","is_error":true}]},"sessionId":"session-exit-plan"}"#,
        ],
    );

    let imported = load_claude_local_history_from_projects_root(
        &root,
        cwd,
        &ClaudeLaunchMode::Resume("session-exit-plan".to_string()),
    )
    .expect("import")
    .expect("history");

    let pending = imported
        .pending_approval_request
        .as_deref()
        .expect("pending approval request");
    let parsed: Value = serde_json::from_str(pending).expect("parse request");
    assert_eq!(
        parsed["method"].as_str(),
        Some("claude/exitPlan/requestApproval")
    );
    assert_eq!(parsed["params"]["toolUseId"].as_str(), Some("toolu_exit"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn local_claude_history_clears_pending_exit_plan_when_session_progresses() {
    let root = temp_test_dir("claude-history-exit-plan-cleared");
    let cwd = Path::new("/repo");
    write_session_file(
        &root,
        cwd,
        "session-exit-plan-cleared",
        &[
            r##"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_exit","name":"ExitPlanMode","input":{"plan":"# Plan\nShip it"}}]},"sessionId":"session-exit-plan-cleared"}"##,
            r#"{"type":"user","message":{"role":"user","content":[{"tool_use_id":"toolu_exit","type":"tool_result","content":"Exit plan mode?","is_error":true}]},"sessionId":"session-exit-plan-cleared"}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Revising the plan."}]},"sessionId":"session-exit-plan-cleared"}"#,
        ],
    );

    let imported = load_claude_local_history_from_projects_root(
        &root,
        cwd,
        &ClaudeLaunchMode::Resume("session-exit-plan-cleared".to_string()),
    )
    .expect("import")
    .expect("history");

    assert!(imported.pending_approval_request.is_none());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn local_claude_history_imports_user_assistant_and_tool_items() {
    let root = temp_test_dir("claude-history-import");
    let cwd = Path::new("/repo");
    write_session_file(
        &root,
        cwd,
        "session-123",
        &[
            r#"{"type":"user","message":{"role":"user","content":"hello"},"sessionId":"session-123"}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"hi there"},{"type":"tool_use","id":"toolu_1","name":"Bash","input":{"command":"pwd","description":"Print working directory"}}]},"sessionId":"session-123"}"#,
            r#"{"type":"user","message":{"role":"user","content":[{"tool_use_id":"toolu_1","type":"tool_result","content":"/repo","is_error":false}]},"toolUseResult":{"stdout":"/repo","stderr":"","interrupted":false},"sessionId":"session-123"}"#,
        ],
    );

    let imported = load_claude_local_history_from_projects_root(
        &root,
        cwd,
        &ClaudeLaunchMode::Resume("session-123".to_string()),
    )
    .expect("import")
    .expect("history");

    assert_eq!(imported.session_id, "session-123");
    assert_eq!(imported.imported_item_count, 4);
    let items = imported.thread["turns"][0]["items"]
        .as_array()
        .expect("items");
    assert_eq!(items[0]["type"].as_str(), Some("userMessage"));
    assert_eq!(items[1]["type"].as_str(), Some("agentMessage"));
    assert_eq!(items[2]["type"].as_str(), Some("toolCall"));
    assert_eq!(items[3]["type"].as_str(), Some("toolResult"));
    assert!(items[3]["output"].as_str().unwrap_or("").contains("$ pwd"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn local_claude_history_hides_internal_plan_mode_tool_rows() {
    let root = temp_test_dir("claude-history-hide-plan-mode-tools");
    let cwd = Path::new("/repo");
    write_session_file(
        &root,
        cwd,
        "session-plan-tools-hidden",
        &[
            r##"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Plan complete."},{"type":"tool_use","id":"toolu_write","name":"Write","input":{"file_path":"/home/wegel/.claude/plans/test-plan.md","content":"# Plan"}},{"type":"tool_use","id":"toolu_search","name":"ToolSearch","input":{"query":"select:ExitPlanMode","max_results":1}},{"type":"tool_use","id":"toolu_exit","name":"ExitPlanMode","input":{"plan":"# Plan\nShip it"}}]},"sessionId":"session-plan-tools-hidden"}"##,
            r#"{"type":"user","message":{"role":"user","content":[{"tool_use_id":"toolu_write","type":"tool_result","content":"File created successfully","is_error":false},{"tool_use_id":"toolu_search","type":"tool_result","content":[{"type":"tool_reference","tool_name":"ExitPlanMode"}],"is_error":false},{"tool_use_id":"toolu_exit","type":"tool_result","content":"Exit plan mode?","is_error":true}]},"sessionId":"session-plan-tools-hidden"}"#,
        ],
    );

    let imported = load_claude_local_history_from_projects_root(
        &root,
        cwd,
        &ClaudeLaunchMode::Resume("session-plan-tools-hidden".to_string()),
    )
    .expect("import")
    .expect("history");

    let items = imported.thread["turns"][0]["items"]
        .as_array()
        .expect("items");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["type"].as_str(), Some("agentMessage"));
    assert_eq!(items[0]["text"].as_str(), Some("Plan complete."));
    assert!(imported.pending_approval_request.is_some());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn local_claude_history_continue_is_disabled_without_authoritative_session_id() {
    let root = temp_test_dir("claude-history-continue");
    let cwd = Path::new("/repo");
    write_session_file(
        &root,
        cwd,
        "some-session",
        &[
            r#"{"type":"user","message":{"role":"user","content":"ignored"},"sessionId":"some-session"}"#,
        ],
    );

    let imported =
        load_claude_local_history_from_projects_root(&root, cwd, &ClaudeLaunchMode::Continue)
            .expect("import");

    assert!(imported.is_none());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn local_claude_history_returns_none_when_session_file_is_missing() {
    let root = temp_test_dir("claude-history-missing");
    fs::create_dir_all(&root).expect("create root");
    let imported = load_claude_local_history_from_projects_root(
        &root,
        Path::new("/repo"),
        &ClaudeLaunchMode::Resume("missing-session".to_string()),
    )
    .expect("import");
    assert!(imported.is_none());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn local_claude_history_returns_none_for_malformed_jsonl() {
    let root = temp_test_dir("claude-history-malformed");
    let cwd = Path::new("/repo");
    write_session_file(&root, cwd, "broken-session", &["{not-json"]);

    let imported = load_claude_local_history_from_projects_root(
        &root,
        cwd,
        &ClaudeLaunchMode::Resume("broken-session".to_string()),
    )
    .expect("import");

    assert!(imported.is_none());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn local_claude_history_returns_none_for_partially_malformed_jsonl() {
    let root = temp_test_dir("claude-history-partial-malformed");
    let cwd = Path::new("/repo");
    write_session_file(
        &root,
        cwd,
        "broken-session",
        &[
            r#"{"type":"user","message":{"role":"user","content":"hello"},"sessionId":"broken-session"}"#,
            "{not-json",
        ],
    );

    let imported = load_claude_local_history_from_projects_root(
        &root,
        cwd,
        &ClaudeLaunchMode::Resume("broken-session".to_string()),
    )
    .expect("import");

    assert!(imported.is_none());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn local_claude_history_returns_none_when_session_file_cannot_be_opened() {
    let root = temp_test_dir("claude-history-unreadable");
    let cwd = Path::new("/repo");
    let project_dir = root.join(claude_project_dir_name(cwd));
    fs::create_dir_all(project_dir.join("broken-session.jsonl"))
        .expect("create directory in place of session file");

    let imported = load_claude_local_history_from_projects_root(
        &root,
        cwd,
        &ClaudeLaunchMode::Resume("broken-session".to_string()),
    )
    .expect("import");

    assert!(imported.is_none());
    let _ = fs::remove_dir_all(root);
}
