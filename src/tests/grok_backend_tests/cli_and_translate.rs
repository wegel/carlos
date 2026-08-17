// CLI, command, catalog, and ACP translation tests for the Grok backend.

#[test]
fn parse_cli_args_supports_grok_backend_flag() {
    let parsed = parse_cli_args(vec![
        "--backend".to_string(),
        "grok".to_string(),
        "continue".to_string(),
    ])
    .expect("parse");

    assert_eq!(parsed.backend, Backend::Grok);
    assert!(parsed.mode_continue);
}

#[test]
fn build_grok_command_uses_agent_stdio_always_approve() {
    let command = build_grok_command_for_test(Path::new("/repo"), &GrokRuntimeSettings::default());
    let args = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(args, ["agent", "--always-approve", "--no-leader", "stdio"]);
}

#[test]
fn build_grok_command_includes_model_and_effort() {
    let command = build_grok_command_for_test(
        Path::new("/repo"),
        &GrokRuntimeSettings::new(Some("grok-4.5".to_string()), Some("medium".to_string())),
    );
    let args = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert!(args
        .windows(2)
        .any(|window| window == ["--model", "grok-4.5"]));
    assert!(args
        .windows(2)
        .any(|window| window == ["--reasoning-effort", "medium"]));
}

#[test]
fn grok_model_catalog_includes_current_models() {
    let catalog = grok_model_catalog();
    let models = catalog
        .iter()
        .map(|model| model.model.as_str())
        .collect::<Vec<_>>();
    assert_eq!(models.first().copied(), Some("grok-4.6"));
    assert!(catalog.first().is_some_and(|model| model.is_default));
    assert!(models.contains(&"grok-4.5"));
    assert_eq!(
        catalog[0].supported_efforts,
        ["xhigh", "high", "medium", "low"]
    );
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
    lines.extend(finish_grok_prompt_turn_for_test(&mut state, false).lines);

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
    let mut live = GrokTranslationState::default();
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
