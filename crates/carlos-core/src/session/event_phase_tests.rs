//! Keep completed root turns idle while their native nonblocking callbacks wait independently.

use carlos_proto::session::Phase;
use serde_json::{json, Value};

use super::SessionState;

fn question(blocking: bool, id: &str) -> Value {
    json!({"id":id,"params":{"threadId":"child","turnId":"child-turn","itemId":"ask",
        "isBlocking":blocking,"questions":[{"id":"field","header":"Choice","question":"Choose","isOther":true}]}})
}

#[test]
fn nonblocking_callbacks_keep_root_running_or_idle_and_queued_blocking_cards_take_precedence() {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.enable_codex();
    session.accept_codex_callback(
        "item/tool/requestUserInput",
        &question(false, "nonblocking"),
    );
    assert_eq!(session.event_phase(), Phase::Idle);
    assert_eq!(session.export_approvals().len(), 1);
    session.set_active_turn_id(Some("root-turn".into()));
    assert_eq!(session.event_phase(), Phase::Running);
    session.accept_codex_callback("item/tool/requestUserInput", &question(true, "blocking"));
    assert_eq!(
        session.event_phase(),
        Phase::Approval,
        "blocking queued cards count even when the active card is nonblocking"
    );
    session.set_active_turn_id(None);
    assert_eq!(session.event_phase(), Phase::Approval);
    session.retire_codex_callback(&json!("blocking"));
    assert_eq!(session.event_phase(), Phase::Idle);
    assert_eq!(
        session.export_approvals().len(),
        1,
        "idle does not retire a native callback"
    );
}
