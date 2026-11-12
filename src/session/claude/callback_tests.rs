//! Drive callback cards and plain question descriptions without a terminal.

use serde_json::json;

use super::callback_questions::ClaudeQuestion;
use super::callbacks::ClaudeCallback;
use crate::session::approval_reply::ApprovalCleared;
use crate::session::state::SessionState;

#[test]
fn callback_commands_keep_exact_cards_without_an_app() {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    for id in ["first", "second"] {
        assert!(matches!(
            session.receive_claude_callback(&json!({
                "callbackId":id, "kind":"permission", "payload":{"toolName":"Read"}
            })),
            ClaudeCallback::Accepted { .. }
        ));
    }
    let first = session.approval().pending().expect("first").entry_id();
    assert!(matches!(
        session.receive_claude_callback(&json!({
            "callbackId":"first", "kind":"permission", "payload":{"toolName":"Write"}
        })),
        ClaudeCallback::Ignored
    ));
    assert_eq!(
        session.approval().pending().expect("unchanged").entry_id(),
        first
    );
    let (_, cleared) = session.cancel_claude_callback(&json!("first"));
    assert!(
        matches!(cleared, Some(ApprovalCleared::Other { editor: Some(id), .. }) if id == "first")
    );
    assert_eq!(
        session
            .approval()
            .pending()
            .expect("promoted")
            .facts()
            .request_id,
        "second"
    );
    assert!(session.cancel_claude_callback(&json!(99)).1.is_none());
}

#[test]
fn question_descriptions_keep_unsupported_precedence_and_schema_defaults() {
    for (kind, payload, error, fields, defaults) in [
        (
            "permission",
            json!({"toolName":"ExitPlanMode","input":{"plan":" "}}),
            Some("Claude supplied no plan text"),
            0,
            false,
        ),
        (
            "elicitation",
            json!({"formIssue":"native issue","requestedSchema":false}),
            Some("native issue"),
            0,
            false,
        ),
        (
            "elicitation",
            json!({"mode":"url","formIssue":"ignored"}),
            None,
            0,
            false,
        ),
        (
            "elicitation",
            json!({"requestedSchema":{"properties":{"x":{"type":"string","default":"offered"}}}}),
            None,
            1,
            true,
        ),
    ] {
        let question = ClaudeQuestion::from_payload(kind, &payload);
        assert_eq!(question.unsupported.as_deref(), error);
        assert_eq!(question.fields.len(), fields);
        assert_eq!(question.defaults, defaults);
    }
}
