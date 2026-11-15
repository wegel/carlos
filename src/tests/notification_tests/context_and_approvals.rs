// Keep native callback parsing and choices until their Core and Device successors land.

#[test]
fn handle_server_request_command_execution_sets_pending_approval() {
    let mut app = AppState::new("thread-1".to_string());

    let action = handle_server_message_line(
        &mut app,
        "{\"jsonrpc\":\"2.0\",\"id\":\"req-1\",\"method\":\"item/commandExecution/requestApproval\",\"params\":{\"itemId\":\"item-1\",\"threadId\":\"thread-1\",\"turnId\":\"turn-1\",\"command\":\"git diff -- src/main.rs\",\"cwd\":\"/repo\",\"reason\":\"needs write access\",\"availableDecisions\":[\"accept\",\"acceptForSession\",\"decline\",\"cancel\"]}}",
    );

    assert!(action.is_none());
    let pending = app.session.approval().pending().expect("pending approval");
    assert_eq!(
        pending.facts().method,
        "item/commandExecution/requestApproval"
    );
    assert_eq!(pending.facts().title, "Approve command execution");
    assert_eq!(pending.facts().detail_lines[0], "git diff -- src/main.rs");
    assert!(pending.facts().can_accept_for_session);
    assert!(pending.facts().can_decline);
    assert!(pending.facts().can_cancel);
}

#[test]
fn permissions_approval_response_allows_turn_or_session_grant() {
    let request = carlos_core::session::approval_state::ApprovalRequestDraft {
        request_id: json!("req-2"),
        method: "item/permissions/requestApproval".to_string(),
        kind: carlos_core::session::approval_state::ApprovalRequestKind::Permissions,
        title: "Grant additional permissions".to_string(),
        detail_lines: vec!["network access".to_string()],
        requested_permissions: Some(json!({"network":{"enabled":true}})),
        can_accept_for_session: true,
        can_decline: true,
        can_cancel: false,
    };

    assert_eq!(
        request.response_for_choice(carlos_core::session::approval_state::ApprovalChoice::Accept),
        Some(json!({"permissions":{"network":{"enabled":true}}}))
    );
    assert_eq!(
        request.response_for_choice(
            carlos_core::session::approval_state::ApprovalChoice::AcceptForSession
        ),
        Some(json!({"permissions":{"network":{"enabled":true}},"scope":"session"}))
    );
    assert_eq!(
        request.response_for_choice(carlos_core::session::approval_state::ApprovalChoice::Decline),
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
    let pending = app.session.approval().pending().expect("pending approval");
    assert_eq!(pending.facts().method, "claude/exitPlan/requestApproval");
    assert_eq!(pending.facts().title, "Approve Claude plan");
    assert_eq!(
        pending.facts().detail_lines[0],
        "Claude wants to exit plan mode and continue with this plan."
    );
    assert!(pending
        .facts()
        .detail_lines
        .iter()
        .any(|line| line == "# Plan"));
    assert!(pending
        .facts()
        .detail_lines
        .iter()
        .any(|line| line == "allowed: run cargo test (Bash)"));
    assert!(!pending.facts().can_accept_for_session);
    assert!(pending.facts().can_decline);
    assert!(!pending.facts().can_cancel);
}

#[test]
fn claude_exit_plan_approval_response_supports_accept_and_decline() {
    let request = carlos_core::session::approval_state::ApprovalRequestDraft {
        request_id: json!({"backend":"claude","kind":"exitPlanMode","toolUseId":"toolu_1"}),
        method: "claude/exitPlan/requestApproval".to_string(),
        kind: carlos_core::session::approval_state::ApprovalRequestKind::ClaudeExitPlanMode,
        title: "Approve Claude plan".to_string(),
        detail_lines: vec!["Do the work".to_string()],
        requested_permissions: None,
        can_accept_for_session: false,
        can_decline: true,
        can_cancel: false,
    };

    assert_eq!(
        request.response_for_choice(carlos_core::session::approval_state::ApprovalChoice::Accept),
        Some(json!({"decision":"accept"}))
    );
    assert_eq!(
        request.response_for_choice(carlos_core::session::approval_state::ApprovalChoice::Decline),
        Some(json!({"decision":"decline"}))
    );
    assert_eq!(
        request.response_for_choice(
            carlos_core::session::approval_state::ApprovalChoice::AcceptForSession
        ),
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
    assert!(app.session.approval().pending().is_none());
}

#[test]
fn turn_completed_keeps_pending_claude_exit_plan_approval() {
    let mut app = AppState::new("thread-1".to_string());
    app.session.set_active_turn_id(Some("turn-1".to_string()));
    app.set_pending_approval(carlos_core::session::approval_state::ApprovalRequestDraft {
        request_id: json!({"backend":"claude","kind":"exitPlanMode","toolUseId":"toolu_1"}),
        method: "claude/exitPlan/requestApproval".to_string(),
        kind: carlos_core::session::approval_state::ApprovalRequestKind::ClaudeExitPlanMode,
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

    assert!(app.session.approval().pending().is_some());
    assert_eq!(*app.session.active_turn_id(), None);
}
