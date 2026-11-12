// Tests native tool rows retained at the UI boundary.

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
fn normalized_common_events_from_a_retired_helper_cannot_touch_the_new_branch() {
    let mut app = AppState::new("old".into());
    handle_notification_line(&mut app, &serde_json::json!({"method":"claude/sessionChanged","params":{"generation":2,"sessionId":"new"}}).to_string());
    for record in [
        serde_json::json!({"method":"item/completed","params":{"generation":1,"item":{"id":"late","type":"agentMessage","text":"old text"}}}),
        serde_json::json!({"method":"turn/completed","params":{"generation":1,"turn":{"id":"old","status":"completed"}}}),
        serde_json::json!({"method":"thread/tokenUsage/updated","params":{"generation":1,"tokenUsage":{"modelContextWindow":100,"total":{"totalTokens":99}}}}),
    ] {
        handle_notification_line(&mut app,&record.to_string());
    }
    assert_eq!(app.thread_id,"new");
    assert!(app.messages.is_empty());
    assert!(app.turn_status_by_id.is_empty());
    assert!(app.context_usage.is_none());
}

#[test]
fn a_fatal_fact_settles_accepted_prompts_before_queued_success_can_arrive() {
    let mut app = AppState::new("session".into());
    for id in ["active","queued"] {
        handle_notification_line(&mut app,&serde_json::json!({"method":"claude/promptSubmitted","params":{"generation":1,"uuid":id,"text":id,"origin":"human"}}).to_string());
    }
    app.record_turn_started("active");
    handle_notification_line(&mut app,&serde_json::json!({"method":"claude/error","params":{"generation":1,"error":{"message":"fixture fatal error"}}}).to_string());
    for record in [
        serde_json::json!({"method":"item/completed","params":{"generation":1,"item":{"id":"late","type":"agentMessage","text":"late success"}}}),
        serde_json::json!({"method":"turn/completed","params":{"generation":1,"turn":{"id":"active","status":"completed"}}}),
    ] {
        handle_notification_line(&mut app,&record.to_string());
    }
    assert!(app.active_turn_id.is_none());
    assert!(app.claude.prompts.entries.is_empty());
    for id in ["active","queued"] {
        assert_eq!(app.turn_status_by_id.get(id),Some(&crate::app::models::TurnStatus::Failed));
    }
    assert!(!app.messages.iter().any(|message| message.text == "late success"));
}
