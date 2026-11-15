//! Exercise native callback routing without a terminal or local editor object.

use serde_json::{json, Value};

use super::callback_update::CallbackEffect;
use super::panel_scope::PanelScope;
use crate::session::actions::ServerRequestAction;
use crate::session::state::SessionState;

#[test]
fn child_callback_is_accepted_during_closed_rebuild_and_resolved_by_owner() {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.close_codex();
    session.set_codex_rebuilding_for_test(true);
    let update = session
        .route_codex_callback(
            "item/commandExecution/requestApproval",
            &request(),
            scope(&session),
        )
        .expect("callback consumed before root filtering");
    assert!(matches!(
        update.effect,
        CallbackEffect::Accepted {
            active: true,
            blocking: true,
            ..
        }
    ));
    let card = session
        .approval()
        .pending()
        .expect("session owns card")
        .entry_id();
    for (owner, retired) in [("root", false), ("child", true)] {
        let frame = json!({"params":{"threadId":owner,"requestId":"request"}});
        let update = session
            .route_codex_callback("serverRequest/resolved", &frame, scope(&session))
            .expect("resolution consumed");
        assert_eq!(matches!(update.effect, CallbackEffect::Retired(_)), retired);
        assert_eq!(session.approval().pending().is_none(), retired);
    }
    assert!(
        session.approval_answer(card).is_none(),
        "old card cannot reply"
    );
}

#[test]
fn form_preflight_and_stale_worker_reply_need_no_editor() {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    let schema = json!({"type":"object","properties":{}});
    let frame = json!({"id":"form","params":{"threadId":"child","serverName":"server",
        "mode":"form","requestedSchema":schema}});
    let update = session
        .route_codex_callback("mcpServer/elicitation/request", &frame, scope(&session))
        .expect("form consumed");
    match update.action.expect("preflight action") {
        ServerRequestAction::ValidateCodexForm {
            id,
            schema: actual,
            answer,
            revision,
        } => {
            assert_eq!(id, "form");
            assert_eq!(actual, schema);
            assert!(answer.is_none());
            assert_eq!(revision, 0);
        }
        _ => panic!("expected form preflight"),
    }
    let update = session
        .route_codex_callback(
            "carlos/codex/formChecked",
            &json!({"params":{"requestId":"form","revision":99,"ok":true}}),
            scope(&session),
        )
        .expect("stale worker consumed");
    assert!(update.action.is_none());
    assert!(matches!(update.effect, CallbackEffect::None));
    assert!(
        session.approval().pending().is_some(),
        "stale work retains card"
    );
}

#[test]
fn preflight_errors_export_bounded_metadata_without_replacing_the_original_card_or_schema() {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    let schema = json!({"type":"object","properties":{"secret":{"type":"string"}}});
    let frame = json!({"id":"form","params":{"threadId":"child","serverName":"server",
        "mode":"form","requestedSchema":schema}});
    session.route_codex_callback("mcpServer/elicitation/request", &frame, scope(&session));
    let original = session.export_approvals()[0].clone();
    let revision = session.approval().revision;
    session.checked_codex_callback(
        &json!({"requestId":"form","revision":0,"ok":false,"error":"é".repeat(3000)}),
    );
    let checked = session.export_approvals()[0].clone();
    assert_eq!(checked.id, original.id);
    assert_eq!(checked.input, original.input);
    assert_eq!(checked.choices, original.choices);
    assert_eq!(
        checked.input_error.as_ref().expect("checked error").len(),
        2048
    );
    assert!(session.approval().revision > revision);
    session.checked_codex_callback(&json!({"requestId":"form","revision":99,"ok":true}));
    assert_eq!(
        session.export_approvals()[0],
        checked,
        "a stale answer check cannot erase preflight facts"
    );
    session.retire_codex_callback(&json!("form"));
    session.checked_codex_callback(
        &json!({"requestId":"form","revision":0,"ok":false,"error":"late"}),
    );
    assert!(
        session.export_approvals().is_empty(),
        "retired checks cannot recreate a callback"
    );
}

fn scope(session: &SessionState) -> PanelScope<'static> {
    PanelScope {
        native: session.codex_scope(),
        generation: session.codex_connection().generation(),
        rebuilding: session.codex_connection().rebuilding(),
        visible: false,
        detail: false,
        selected: None,
    }
}

fn request() -> Value {
    json!({"id":"request","params":{"threadId":"child","turnId":"turn",
        "itemId":"command","command":"pwd"}})
}
