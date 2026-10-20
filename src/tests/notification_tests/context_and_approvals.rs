// Tests for context-usage updates and approval-request notifications.

#[test]
fn handle_notification_updates_context_usage_when_present() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"turn/completed\",\"params\":{\"usage\":{\"context_window\":256000,\"context_tokens\":128000}}}",
        );

    assert_eq!(app.context_usage, None);
}

#[test]
fn handle_notification_thread_initialized_updates_thread_id() {
    let mut app = AppState::new("claude-pending-session".to_string());

    handle_notification_line(
        &mut app,
        "{\"method\":\"thread/initialized\",\"params\":{\"thread\":{\"id\":\"session-123\"},\"model\":\"claude-opus-4-6\"}}",
    );

    assert_eq!(app.thread_id, "session-123");
    assert_eq!(app.runtime_settings_label(), "claude-opus-4-6");
}

#[test]
fn handle_notification_thread_initialized_clears_context_usage_when_thread_changes() {
    let mut app = AppState::new("claude-pending-session".to_string());
    app.context_usage = Some(ContextUsage {
        used: 1_000_000,
        max: 1_000_000,
    });

    handle_notification_line(
        &mut app,
        "{\"method\":\"thread/initialized\",\"params\":{\"thread\":{\"id\":\"session-123\"},\"model\":\"claude-opus-4-6\"}}",
    );

    assert_eq!(app.thread_id, "session-123");
    assert_eq!(app.context_usage, None);
}

#[test]
fn handle_notification_thread_initialized_preserves_context_usage_for_same_thread() {
    let mut app = AppState::new("session-123".to_string());
    app.context_usage = Some(ContextUsage {
        used: 80_495,
        max: 1_000_000,
    });

    handle_notification_line(
        &mut app,
        "{\"method\":\"thread/initialized\",\"params\":{\"thread\":{\"id\":\"session-123\"},\"model\":\"claude-opus-4-6\"}}",
    );

    assert_eq!(
        app.context_usage,
        Some(ContextUsage {
            used: 80_495,
            max: 1_000_000,
        })
    );
}

#[test]
fn handle_notification_thread_initialized_preserves_existing_effort_when_only_model_arrives() {
    let mut app = AppState::new("claude-pending-session".to_string());
    app.set_runtime_settings(None, Some("high".to_string()), None);

    handle_notification_line(
        &mut app,
        "{\"method\":\"thread/initialized\",\"params\":{\"thread\":{\"id\":\"session-123\"},\"model\":\"claude-opus-4-6\"}}",
    );

    assert_eq!(app.thread_id, "session-123");
    assert_eq!(app.runtime_settings_label(), "claude-opus-4-6/high");
}

#[test]
fn handle_notification_thread_compacted_clears_context_usage() {
    let mut app = AppState::new("thread-1".to_string());
    app.context_usage = Some(ContextUsage {
        used: 80_495,
        max: 1_000_000,
    });

    handle_notification_line(
        &mut app,
        "{\"method\":\"thread/compacted\",\"params\":{\"threadId\":\"thread-1\"}}",
    );

    assert_eq!(app.context_usage, None);
    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].text, "↻ Context compacted");
}

#[test]
fn handle_notification_turn_completed_interrupted_appends_system_message() {
    let mut app = AppState::new("thread-1".to_string());
    app.active_turn_id = Some("turn-1".to_string());

    handle_notification_line(
        &mut app,
        "{\"method\":\"turn/completed\",\"params\":{\"threadId\":\"thread-1\",\"turn\":{\"id\":\"turn-1\",\"status\":\"interrupted\"}}}",
    );

    assert_eq!(app.active_turn_id, None);
    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::System);
    assert_eq!(app.messages[0].text, "Turn interrupted");
}

#[test]
fn incoming_agent_delta_does_not_reenable_auto_follow_when_scrolled_up() {
    let mut app = AppState::new("thread-1".to_string());
    let idx = app.append_message(Role::Assistant, "hello");
    app.put_item_mapping("item-1", idx);
    app.viewport.auto_follow_bottom = false;
    app.viewport.scroll_top = 4;

    handle_notification_line(
        &mut app,
        "{\"method\":\"item/agentMessage/delta\",\"params\":{\"itemId\":\"item-1\",\"delta\":\" world\"}}",
    );

    assert!(!app.viewport.auto_follow_bottom);
    assert_eq!(app.viewport.scroll_top, 4);
    assert_eq!(app.messages[idx].text, "hello world");
}

#[test]
fn handle_server_request_command_execution_sets_pending_approval() {
    let mut app = AppState::new("thread-1".to_string());

    let action = handle_server_message_line(
        &mut app,
        "{\"jsonrpc\":\"2.0\",\"id\":\"req-1\",\"method\":\"item/commandExecution/requestApproval\",\"params\":{\"itemId\":\"item-1\",\"threadId\":\"thread-1\",\"turnId\":\"turn-1\",\"command\":\"git diff -- src/main.rs\",\"cwd\":\"/repo\",\"reason\":\"needs write access\",\"availableDecisions\":[\"accept\",\"acceptForSession\",\"decline\",\"cancel\"]}}",
    );

    assert!(action.is_none());
    let pending = app.approval.pending.expect("pending approval");
    assert_eq!(pending.method, "item/commandExecution/requestApproval");
    assert_eq!(pending.title, "Approve command execution");
    assert_eq!(pending.detail_lines[0], "git diff -- src/main.rs");
    assert!(pending.can_accept_for_session);
    assert!(pending.can_decline);
    assert!(pending.can_cancel);
}

#[test]
fn permissions_approval_response_allows_turn_or_session_grant() {
    let request = super::state::PendingApprovalRequest {
        request_id: json!("req-2"),
        method: "item/permissions/requestApproval".to_string(),
        kind: super::state::ApprovalRequestKind::Permissions,
        title: "Grant additional permissions".to_string(),
        detail_lines: vec!["network access".to_string()],
        requested_permissions: Some(json!({"network":{"enabled":true}})),
        can_accept_for_session: true,
        can_decline: true,
        can_cancel: false,
    };

    assert_eq!(
        request.response_for_choice(super::state::ApprovalChoice::Accept),
        Some(json!({"permissions":{"network":{"enabled":true}}}))
    );
    assert_eq!(
        request.response_for_choice(super::state::ApprovalChoice::AcceptForSession),
        Some(json!({"permissions":{"network":{"enabled":true}},"scope":"session"}))
    );
    assert_eq!(
        request.response_for_choice(super::state::ApprovalChoice::Decline),
        Some(json!({"permissions":{}}))
    );
}

#[test]
fn handle_server_request_claude_exit_plan_sets_pending_approval() {
    let mut app = AppState::new("thread-1".to_string());

    let action = handle_server_message_line(
        &mut app,
        "{\"jsonrpc\":\"2.0\",\"id\":{\"backend\":\"claude\",\"kind\":\"exitPlanMode\",\"toolUseId\":\"toolu_1\"},\"method\":\"claude/exitPlan/requestApproval\",\"params\":{\"toolUseId\":\"toolu_1\",\"plan\":\"# Plan\\nDo the work\",\"planFilePath\":\"/tmp/plan.md\",\"allowedPrompts\":[{\"prompt\":\"run cargo test\",\"tool\":\"Bash\"}]}}",
    );

    assert!(action.is_none());
    let pending = app.approval.pending.expect("pending approval");
    assert_eq!(pending.method, "claude/exitPlan/requestApproval");
    assert_eq!(pending.title, "Approve Claude plan");
    assert_eq!(
        pending.detail_lines[0],
        "Claude wants to exit plan mode and continue with this plan."
    );
    assert!(pending.detail_lines.iter().any(|line| line == "# Plan"));
    assert!(pending
        .detail_lines
        .iter()
        .any(|line| line == "allowed: run cargo test (Bash)"));
    assert!(!pending.can_accept_for_session);
    assert!(pending.can_decline);
    assert!(!pending.can_cancel);
}

#[test]
fn claude_exit_plan_approval_response_supports_accept_and_decline() {
    let request = super::state::PendingApprovalRequest {
        request_id: json!({"backend":"claude","kind":"exitPlanMode","toolUseId":"toolu_1"}),
        method: "claude/exitPlan/requestApproval".to_string(),
        kind: super::state::ApprovalRequestKind::ClaudeExitPlanMode,
        title: "Approve Claude plan".to_string(),
        detail_lines: vec!["Do the work".to_string()],
        requested_permissions: None,
        can_accept_for_session: false,
        can_decline: true,
        can_cancel: false,
    };

    assert_eq!(
        request.response_for_choice(super::state::ApprovalChoice::Accept),
        Some(json!({"decision":"accept"}))
    );
    assert_eq!(
        request.response_for_choice(super::state::ApprovalChoice::Decline),
        Some(json!({"decision":"decline"}))
    );
    assert_eq!(
        request.response_for_choice(super::state::ApprovalChoice::AcceptForSession),
        None
    );
}

#[test]
fn unsupported_server_request_returns_jsonrpc_error_action() {
    let mut app = AppState::new("thread-1".to_string());

    let action = handle_server_message_line(
        &mut app,
        "{\"jsonrpc\":\"2.0\",\"id\":99,\"method\":\"item/tool/requestUserInput\",\"params\":{\"itemId\":\"item-1\"}}",
    );

    match action {
        Some(ServerRequestAction::ReplyError {
            request_id,
            code,
            message,
        }) => {
            assert_eq!(request_id, json!(99));
            assert_eq!(code, -32601);
            assert!(message.contains("unsupported server request"));
        }
        _ => panic!("expected reply error"),
    }
    assert!(app.approval.pending.is_none());
}

#[test]
fn turn_completed_keeps_pending_claude_exit_plan_approval() {
    let mut app = AppState::new("thread-1".to_string());
    app.active_turn_id = Some("turn-1".to_string());
    app.set_pending_approval(super::state::PendingApprovalRequest {
        request_id: json!({"backend":"claude","kind":"exitPlanMode","toolUseId":"toolu_1"}),
        method: "claude/exitPlan/requestApproval".to_string(),
        kind: super::state::ApprovalRequestKind::ClaudeExitPlanMode,
        title: "Approve Claude plan".to_string(),
        detail_lines: vec!["Do the work".to_string()],
        requested_permissions: None,
        can_accept_for_session: false,
        can_decline: true,
        can_cancel: false,
    });

    handle_notification_line(
        &mut app,
        "{\"method\":\"turn/completed\",\"params\":{\"threadId\":\"thread-1\",\"turn\":{\"id\":\"turn-1\",\"status\":\"completed\"}}}",
    );

    assert!(app.approval.pending.is_some());
    assert_eq!(app.active_turn_id, None);
}

#[test]
fn append_turn_interrupted_marker_is_deduplicated() {
    let mut app = AppState::new("thread-1".to_string());
    app.append_turn_interrupted_marker();
    app.append_turn_interrupted_marker();
    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::System);
    assert_eq!(app.messages[0].text, "Turn interrupted");
}

#[test]
fn handle_notification_thread_token_usage_updated_sets_context_usage() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"thread/tokenUsage/updated\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-1\",\"tokenUsage\":{\"modelContextWindow\":256000,\"last\":{\"cachedInputTokens\":0,\"inputTokens\":0,\"outputTokens\":0,\"reasoningOutputTokens\":0,\"totalTokens\":0},\"total\":{\"cachedInputTokens\":0,\"inputTokens\":100000,\"outputTokens\":22000,\"reasoningOutputTokens\":0,\"totalTokens\":122000}}}}",
        );

    assert_eq!(
        app.context_usage,
        Some(ContextUsage {
            used: 122_000,
            max: 256_000
        })
    );
}

#[test]
fn handle_notification_thread_token_usage_prefers_last_when_total_exceeds_window() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"thread/tokenUsage/updated\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-1\",\"tokenUsage\":{\"modelContextWindow\":258400,\"last\":{\"cachedInputTokens\":0,\"inputTokens\":46000,\"outputTokens\":300,\"reasoningOutputTokens\":37,\"totalTokens\":46337},\"total\":{\"cachedInputTokens\":0,\"inputTokens\":301000,\"outputTokens\":2104,\"reasoningOutputTokens\":30,\"totalTokens\":303134}}}}",
        );

    assert_eq!(
        app.context_usage,
        Some(ContextUsage {
            used: 46_337,
            max: 258_400
        })
    );
}
