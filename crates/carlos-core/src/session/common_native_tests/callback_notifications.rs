//! Retain original native callback facts, reply choices and turn-completion lifetimes in Core.

use serde_json::json;

use super::{feed as handle_server_message_line, session};
use crate::session::actions::ServerRequestAction;
use crate::session::approval_state::{ApprovalChoice, ApprovalRequestDraft, ApprovalRequestKind};

#[test]
fn handle_server_request_command_execution_sets_pending_approval() {
    let mut app = session("thread-1");

    let action = handle_server_message_line(
        &mut app,
        "{\"jsonrpc\":\"2.0\",\"id\":\"req-1\",\"method\":\"item/commandExecution/requestApproval\",\"params\":{\"itemId\":\"item-1\",\"threadId\":\"thread-1\",\"turnId\":\"turn-1\",\"command\":\"git diff -- src/main.rs\",\"cwd\":\"/repo\",\"reason\":\"needs write access\",\"availableDecisions\":[\"accept\",\"acceptForSession\",\"decline\",\"cancel\"]}}",
    );

    assert!(action.is_none());
    let pending = app.approval().pending().expect("pending approval");
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
    let request = ApprovalRequestDraft {
        request_id: json!("req-2"),
        method: "item/permissions/requestApproval".to_string(),
        kind: ApprovalRequestKind::Permissions,
        title: "Grant additional permissions".to_string(),
        detail_lines: vec!["network access".to_string()],
        requested_permissions: Some(json!({"network":{"enabled":true}})),
        can_accept_for_session: true,
        can_decline: true,
        can_cancel: false,
    };

    assert_eq!(
        request.response_for_choice(ApprovalChoice::Accept),
        Some(json!({"permissions":{"network":{"enabled":true}}}))
    );
    assert_eq!(
        request.response_for_choice(ApprovalChoice::AcceptForSession),
        Some(json!({"permissions":{"network":{"enabled":true}},"scope":"session"}))
    );
    assert_eq!(
        request.response_for_choice(ApprovalChoice::Decline),
        Some(json!({"permissions":{}}))
    );
}

#[test]
fn handle_server_request_claude_exit_plan_sets_pending_approval() {
    let mut app = session("thread-1");

    let action = handle_server_message_line(
        &mut app,
        "{\"jsonrpc\":\"2.0\",\"id\":{\"backend\":\"claude\",\"kind\":\"exitPlanMode\",\"toolUseId\":\"toolu_1\"},\"method\":\"claude/exitPlan/requestApproval\",\"params\":{\"toolUseId\":\"toolu_1\",\"plan\":\"# Plan\\nDo the work\",\"planFilePath\":\"/tmp/plan.md\",\"allowedPrompts\":[{\"prompt\":\"run cargo test\",\"tool\":\"Bash\"}]}}",
    );

    assert!(action.is_none());
    let pending = app.approval().pending().expect("pending approval");
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
    let request = ApprovalRequestDraft {
        request_id: json!({"backend":"claude","kind":"exitPlanMode","toolUseId":"toolu_1"}),
        method: "claude/exitPlan/requestApproval".to_string(),
        kind: ApprovalRequestKind::ClaudeExitPlanMode,
        title: "Approve Claude plan".to_string(),
        detail_lines: vec!["Do the work".to_string()],
        requested_permissions: None,
        can_accept_for_session: false,
        can_decline: true,
        can_cancel: false,
    };

    assert_eq!(
        request.response_for_choice(ApprovalChoice::Accept),
        Some(json!({"decision":"accept"}))
    );
    assert_eq!(
        request.response_for_choice(ApprovalChoice::Decline),
        Some(json!({"decision":"decline"}))
    );
    assert_eq!(
        request.response_for_choice(ApprovalChoice::AcceptForSession),
        None
    );
}

#[test]
fn unsupported_server_request_returns_jsonrpc_error_action() {
    let mut app = session("thread-1");

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
    assert!(app.approval().pending().is_none());
}

#[test]
fn turn_completed_keeps_pending_claude_exit_plan_approval() {
    let mut app = session("thread-1");
    app.set_active_turn_id(Some("turn-1".to_string()));
    let _ = app.set_pending_approval(ApprovalRequestDraft {
        request_id: json!({"backend":"claude","kind":"exitPlanMode","toolUseId":"toolu_1"}),
        method: "claude/exitPlan/requestApproval".to_string(),
        kind: ApprovalRequestKind::ClaudeExitPlanMode,
        title: "Approve Claude plan".to_string(),
        detail_lines: vec!["Do the work".to_string()],
        requested_permissions: None,
        can_accept_for_session: false,
        can_decline: true,
        can_cancel: false,
    });

    let _ = handle_server_message_line(
        &mut app,
        "{\"method\":\"turn/completed\",\"params\":{\"threadId\":\"thread-1\",\"turn\":{\"id\":\"turn-1\",\"status\":\"completed\"}}}",
    );

    assert!(app.approval().pending().is_some());
    assert_eq!(*app.active_turn_id(), None);
}

#[test]
fn a_child_question_after_root_completion_keeps_its_own_native_request_and_fields() {
    let mut core = session("root");
    core.enable_codex();
    for frame in [
        json!({"method":"turn/started","params":{"threadId":"root","turn":{"id":"t"}}}),
        json!({"method":"turn/completed","params":{"threadId":"root","turn":{"id":"t","status":"completed"}}}),
        json!({"id":"late","method":"item/tool/requestUserInput","params":{
            "threadId":"child","turnId":"child-turn","itemId":"child-item","questions":[{
            "id":"q","header":"Header","question":"Late question?","options":[]}]}}),
    ] {
        assert!(handle_server_message_line(&mut core, &frame.to_string()).is_none());
    }
    assert!(core.active_turn_id().is_none());
    assert_eq!(
        core.approval()
            .pending()
            .expect("late callback")
            .facts()
            .request_id,
        json!("late")
    );
    let card = core.export_approvals().remove(0);
    assert!(
        matches!(card.input, Some(carlos_proto::session::ApprovalInput::Fields { fields })
        if fields.len() == 1 && fields[0].key == "q")
    );
    assert_eq!(core.codex_callbacks().len(), 1);
}
