//! Check tilde config paths in a child process without mutating parallel tests' environment.

use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

use super::translation_contract_tests::collect_synthetic_lines;

fn assert_isolated_tilde_plan() {
    let root = tempfile::tempdir().expect("private Claude config root");
    let home = root.path().join("home");
    std::fs::create_dir_all(home.join(".claude-tt/plans")).expect("private plan directory");
    let output = Command::new(std::env::current_exe().expect("current test binary"))
        .args(["--exact", "claude_backend::translation_path_tests::translate_claude_hides_plan_write_tools_under_tilde_config_dir", "--nocapture"])
        .env("HOME", home)
        .env("CLAUDE_CONFIG_DIR", "~/.claude-tt")
        .env("CARLOS_TEST_TILDE_PLAN_CHILD", "1")
        .output().expect("run isolated translator test");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("1 passed"),
        "child executed the exact original test"
    );
}

#[test]
fn translate_claude_hides_plan_write_tools_under_tilde_config_dir() {
    if std::env::var_os("CARLOS_TEST_TILDE_PLAN_CHILD").is_none() {
        assert_isolated_tilde_plan();
        return;
    }
    let home = PathBuf::from(std::env::var_os("HOME").expect("private child home"));
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
}
