//! Preserve native ACP text, tools, replay and completion rules through the real translator.

use serde_json::{json, Value};

use super::{finish_prompt_turn, translate_grok_notification};
use crate::grok_backend::GrokTranslationState;

fn collect_lines(state: &mut GrokTranslationState, events: &[(&str, Value)]) -> Vec<String> {
    let mut out = Vec::new();
    for (method, params) in events {
        out.extend(translate_grok_notification(state, method, params).lines);
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

#[test]
fn translate_grok_streams_text_and_completes_turn() {
    let mut state = GrokTranslationState::default();
    let mut lines = collect_lines(
        &mut state,
        &[
            (
                "session/update",
                json!({
                    "sessionId": "sess-1",
                    "update": {
                        "sessionUpdate": "agent_message_chunk",
                        "content": { "type": "text", "text": "Hello" }
                    }
                }),
            ),
            (
                "session/update",
                json!({
                    "sessionId": "sess-1",
                    "update": {
                        "sessionUpdate": "agent_message_chunk",
                        "content": { "type": "text", "text": " world" }
                    }
                }),
            ),
        ],
    );
    lines.extend(finish_prompt_turn(&mut state, false).lines);

    let methods = methods(&lines);
    assert!(methods.contains(&"turn/started".to_string()));
    assert!(methods.contains(&"item/started".to_string()));
    assert!(methods.contains(&"item/agentMessage/delta".to_string()));
    assert!(methods.contains(&"item/completed".to_string()));
    assert!(methods.contains(&"turn/completed".to_string()));
}

#[test]
fn translate_grok_hides_read_tool_results() {
    let mut state = GrokTranslationState::default();
    let lines = collect_lines(
        &mut state,
        &[
            (
                "session/update",
                json!({
                    "update": {
                        "sessionUpdate": "tool_call",
                        "toolCallId": "call-1",
                        "title": "read_file",
                        "kind": "read",
                        "rawInput": { "path": "src/main.rs" }
                    }
                }),
            ),
            (
                "session/update",
                json!({
                    "update": {
                        "sessionUpdate": "tool_call_update",
                        "toolCallId": "call-1",
                        "status": "completed",
                        "content": [{
                            "type": "content",
                            "content": { "type": "text", "text": "fn main() {}" }
                        }]
                    }
                }),
            ),
        ],
    );
    let completed: Vec<Value> = lines
        .iter()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|value| value.get("method").and_then(Value::as_str) == Some("item/completed"))
        .collect();
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0]["params"]["item"]["type"], "toolCall");
    let rendered = serde_json::to_string(&completed).expect("serialize");
    assert!(!rendered.contains("fn main() {}"));
}

#[test]
fn translate_grok_emits_execute_tool_result() {
    let mut state = GrokTranslationState::default();
    let lines = collect_lines(
        &mut state,
        &[
            (
                "session/update",
                json!({
                    "update": {
                        "sessionUpdate": "tool_call",
                        "toolCallId": "call-2",
                        "title": "run_terminal_command",
                        "kind": "execute",
                        "rawInput": { "command": "git status" }
                    }
                }),
            ),
            (
                "session/update",
                json!({
                    "update": {
                        "sessionUpdate": "tool_call_update",
                        "toolCallId": "call-2",
                        "status": "completed",
                        "rawOutput": { "output": "clean tree" }
                    }
                }),
            ),
        ],
    );
    let completed: Vec<Value> = lines
        .iter()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|value| value.get("method").and_then(Value::as_str) == Some("item/completed"))
        .collect();
    assert_eq!(completed.len(), 2);
    assert_eq!(completed[0]["params"]["item"]["type"], "toolCall");
    assert_eq!(completed[1]["params"]["item"]["type"], "toolResult");
    assert_eq!(completed[1]["params"]["item"]["text"], "clean tree");
}

#[test]
fn translate_grok_replays_user_chunks_only_while_replaying() {
    let mut live = GrokTranslationState::suppressing_user_echo();
    let live_lines = collect_lines(
        &mut live,
        &[(
            "session/update",
            json!({
                "update": {
                    "sessionUpdate": "user_message_chunk",
                    "content": { "type": "text", "text": "hello" }
                }
            }),
        )],
    );
    assert!(live_lines.is_empty());

    let mut replay = GrokTranslationState::replaying();
    let replay_lines = collect_lines(
        &mut replay,
        &[
            (
                "session/update",
                json!({
                    "update": {
                        "sessionUpdate": "user_message_chunk",
                        "content": { "type": "text", "text": "hello" }
                    }
                }),
            ),
            (
                "session/update",
                json!({
                    "update": {
                        "sessionUpdate": "agent_message_chunk",
                        "content": { "type": "text", "text": "hi" }
                    }
                }),
            ),
        ],
    );
    let started: Vec<Value> = replay_lines
        .iter()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|value| value.get("method").and_then(Value::as_str) == Some("item/started"))
        .collect();
    assert_eq!(started[0]["params"]["item"]["type"], "userMessage");
}

#[test]
fn translate_grok_adopts_injected_user_prompt_as_new_turn() {
    let mut state = GrokTranslationState::default();
    let lines = collect_lines(
        &mut state,
        &[(
            "session/update",
            json!({
                "update": {
                    "sessionUpdate": "user_message_chunk",
                    "content": { "type": "text", "text": "<system-reminder>done" }
                }
            }),
        )],
    );
    let methods = methods(&lines);
    assert!(methods.contains(&"turn/started".to_string()));
    assert!(methods.contains(&"item/started".to_string()));
}

#[test]
fn translate_grok_emits_turn_completed_once() {
    let mut state = GrokTranslationState::default();
    let mut lines = collect_lines(
        &mut state,
        &[
            (
                "session/update",
                json!({
                    "update": {
                        "sessionUpdate": "agent_message_chunk",
                        "content": { "type": "text", "text": "hi" }
                    }
                }),
            ),
            (
                "_x.ai/session/update",
                json!({
                    "update": {
                        "sessionUpdate": "turn_completed",
                        "stop_reason": "end_turn"
                    }
                }),
            ),
        ],
    );
    lines.extend(finish_prompt_turn(&mut state, false).lines);
    let completed = methods(&lines)
        .into_iter()
        .filter(|method| method == "turn/completed")
        .count();
    assert_eq!(completed, 1);
}

#[test]
fn translate_grok_drops_stale_chunks_after_interrupt() {
    let mut state = GrokTranslationState::default();
    let started = collect_lines(
        &mut state,
        &[(
            "session/update",
            json!({
                "update": {
                    "sessionUpdate": "agent_message_chunk",
                    "content": { "type": "text", "text": "working" }
                }
            }),
        )],
    );
    assert!(methods(&started).contains(&"turn/started".to_string()));
    let _ = finish_prompt_turn(&mut state, true);
    let stale = collect_lines(
        &mut state,
        &[(
            "session/update",
            json!({
                "update": {
                    "sessionUpdate": "agent_message_chunk",
                    "content": { "type": "text", "text": "leftover" }
                }
            }),
        )],
    );
    assert!(stale.is_empty());
}
