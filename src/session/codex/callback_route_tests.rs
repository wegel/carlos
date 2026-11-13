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
            false,
        )
        .expect("callback consumed before root filtering");
    assert!(matches!(
        update.effect,
        CallbackEffect::Accepted {
            focus: Some(true),
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
            .route_codex_callback("serverRequest/resolved", &frame, scope(&session), true)
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
        .route_codex_callback(
            "mcpServer/elicitation/request",
            &frame,
            scope(&session),
            false,
        )
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
            true,
        )
        .expect("stale worker consumed");
    assert!(update.action.is_none());
    assert!(matches!(update.effect, CallbackEffect::None));
    assert!(
        session.approval().pending().is_some(),
        "stale work retains card"
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
