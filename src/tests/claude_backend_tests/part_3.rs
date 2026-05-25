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
fn local_claude_history_recovers_context_usage_from_hinted_assistant_usage() {
    let root = temp_test_dir("claude-history-context-usage");
    let cwd = Path::new("/repo");
    write_session_file(
        &root,
        cwd,
        "session-context-usage",
        &[
            r#"{"type":"user","message":{"role":"user","content":"hello"},"sessionId":"session-context-usage"}"#,
            r#"{"type":"assistant","message":{"role":"assistant","model":"claude-opus-4-6[1m]","content":[{"type":"text","text":"hi"}],"usage":{"input_tokens":1,"cache_creation_input_tokens":685,"cache_read_input_tokens":79716,"output_tokens":93}},"sessionId":"session-context-usage"}"#,
        ],
    );

    let imported = load_claude_local_history_from_projects_root(
        &root,
        cwd,
        &ClaudeLaunchMode::Resume("session-context-usage".to_string()),
    )
    .expect("import")
    .expect("history");

    let usage = imported.context_usage.expect("context usage");
    assert_eq!(usage.used, 80_495);
    assert_eq!(usage.max, 1_000_000);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn local_claude_history_skips_context_usage_without_authoritative_window() {
    let root = temp_test_dir("claude-history-context-usage-plain");
    let cwd = Path::new("/repo");
    write_session_file(
        &root,
        cwd,
        "session-context-usage-plain",
        &[
            r#"{"type":"assistant","message":{"role":"assistant","model":"claude-opus-4-6","content":[{"type":"text","text":"hi"}],"usage":{"input_tokens":3,"cache_creation_input_tokens":88328,"cache_read_input_tokens":11929,"output_tokens":8}},"sessionId":"session-context-usage-plain"}"#,
        ],
    );

    let imported = load_claude_local_history_from_projects_root(
        &root,
        cwd,
        &ClaudeLaunchMode::Resume("session-context-usage-plain".to_string()),
    )
    .expect("import")
    .expect("history");

    assert!(imported.context_usage.is_none());

    let _ = fs::remove_dir_all(root);
}
