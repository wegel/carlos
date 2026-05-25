// Tests for Claude protocol translation and turn submission.

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
fn translated_claude_init_preserves_selected_runtime_settings() {
    let synthetic = collect_synthetic_lines(&[
        r#"{"type":"system","subtype":"init","session_id":"session-1","model":"claude-opus-4-7"}"#,
    ]);
    let mut app = AppState::new("claude-pending-session".to_string());
    app.set_runtime_settings(
        Some("claude-opus-4-6".to_string()),
        Some("high".to_string()),
        None,
    );

    for line in synthetic {
        handle_notification_line(&mut app, &line);
    }

    assert_eq!(app.thread_id, "session-1");
    assert_eq!(app.runtime_settings_label(), "claude-opus-4-6/high");
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
