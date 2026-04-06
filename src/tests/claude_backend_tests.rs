use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use serde_json::Value;

use super::input_events::{submit_turn_text, submit_turn_text_with_history};
use super::*;
use crate::backend::{BackendClient, BackendKind};
use crate::claude_backend::{
    claude_project_dir_name, load_claude_local_history_from_projects_root, translate_claude_line,
    ClaudeLaunchMode, ClaudeTranslationState,
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
fn submit_turn_text_queues_behind_existing_claude_turns() {
    let client = ClaudeSteerMock;
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
}

#[test]
fn claude_project_dir_name_encodes_current_worktree_path() {
    assert_eq!(
        claude_project_dir_name(Path::new("/var/home/wegel/work/perso/stormvault")),
        "-var-home-wegel-work-perso-stormvault"
    );
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
