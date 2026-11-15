//! Preserve native Claude text snapshots, hidden tools and tool result envelopes.

use serde_json::Value;

use super::translate::translate_claude_line;
use super::types::ClaudeTranslationState;

/// Apply the native translator to an ordered fixed stream without a view.
pub(super) fn collect_synthetic_lines(lines: &[&str]) -> Vec<String> {
    let mut state = ClaudeTranslationState::default();
    lines
        .iter()
        .flat_map(|line| {
            translate_claude_line(&mut state, line)
                .expect("translate native line")
                .lines
        })
        .collect()
}

fn parse_method(line: &str) -> Option<String> {
    serde_json::from_str::<Value>(line)
        .ok()?
        .get("method")?
        .as_str()
        .map(str::to_string)
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
    assert!(!methods.contains(&"thread/tokenUsage/updated".to_string()));
    assert!(methods.contains(&"turn/completed".to_string()));
}

#[test]
fn translate_claude_system_init_does_not_export_backend_model() {
    let synthetic = collect_synthetic_lines(&[
        r#"{"type":"system","subtype":"init","session_id":"session-1","model":"claude-opus-4-7"}"#,
    ]);

    let initialized = synthetic
        .iter()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|value| value.get("method").and_then(Value::as_str) == Some("thread/initialized"))
        .expect("thread initialized");

    assert_eq!(
        initialized["params"]["thread"]["id"].as_str(),
        Some("session-1")
    );
    assert!(initialized["params"].get("model").is_none());
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
    assert_eq!(completed[0]["params"]["item"]["text"].as_str(), Some("OK"));
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
    assert_eq!(completed[0]["params"]["item"]["text"].as_str(), Some("OK"));
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
