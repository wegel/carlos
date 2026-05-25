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
fn translate_claude_result_prefers_model_usage_when_available() {
    let synthetic = collect_synthetic_lines(&[
        r#"{"type":"system","subtype":"init","session_id":"session-1","model":"claude-opus-4-7[1m]"}"#,
        r#"{"type":"result","session_id":"session-1","terminal_reason":"completed","usage":{"input_tokens":3,"cache_creation_input_tokens":544,"cache_read_input_tokens":8762,"output_tokens":4},"modelUsage":{"claude-opus-4-7[1m]":{"inputTokens":1,"outputTokens":93,"cacheReadInputTokens":79716,"cacheCreationInputTokens":685,"contextWindow":1000000}}}"#,
    ]);

    let token_update = synthetic
        .iter()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|value| {
            value.get("method").and_then(Value::as_str) == Some("thread/tokenUsage/updated")
        })
        .expect("token usage update");

    assert_eq!(
        token_update["params"]["tokenUsage"]["total"]["totalTokens"].as_u64(),
        Some(80_495)
    );
    assert_eq!(
        token_update["params"]["tokenUsage"]["modelContextWindow"].as_u64(),
        Some(1_000_000)
    );
}

#[test]
fn translate_claude_usage_without_window_hint_skips_token_update() {
    let synthetic = collect_synthetic_lines(&[
        r#"{"type":"system","subtype":"init","session_id":"session-1","model":"claude-opus-4-6"}"#,
        r#"{"type":"stream_event","event":{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"input_tokens":3,"cache_creation_input_tokens":88328,"cache_read_input_tokens":11929,"output_tokens":8}},"session_id":"session-1"}"#,
    ]);

    let methods: Vec<String> = synthetic
        .iter()
        .filter_map(|line| parse_method(line))
        .collect();
    assert!(!methods.contains(&"thread/tokenUsage/updated".to_string()));
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

#[test]
fn live_claude_item_ids_are_unique_across_process_respawns() {
    let lines = &[
        r#"{"type":"stream_event","event":{"type":"message_start","message":{"id":"msg_1","role":"assistant","type":"message","content":[]}}}"#,
        r#"{"type":"stream_event","event":{"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}}"#,
        r#"{"type":"stream_event","event":{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"hello"}}}"#,
        r#"{"type":"stream_event","event":{"type":"content_block_stop","index":1}}"#,
    ];

    let first = collect_live_forwarded_lines_for_test(lines);
    let second = collect_live_forwarded_lines_for_test(lines);

    let first_item_id = first
        .iter()
        .find_map(|line| {
            let value = serde_json::from_str::<Value>(line).ok()?;
            (value.get("method").and_then(Value::as_str) == Some("item/started"))
                .then(|| value["params"]["item"]["id"].as_str().map(str::to_string))
                .flatten()
        })
        .expect("first item id");
    let second_item_id = second
        .iter()
        .find_map(|line| {
            let value = serde_json::from_str::<Value>(line).ok()?;
            (value.get("method").and_then(Value::as_str) == Some("item/started"))
                .then(|| value["params"]["item"]["id"].as_str().map(str::to_string))
                .flatten()
        })
        .expect("second item id");

    assert_ne!(first_item_id, second_item_id);
}
