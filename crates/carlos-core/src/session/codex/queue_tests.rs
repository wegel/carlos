//! Exercise native queue commands and recovery without an app or editor.

use serde_json::json;

use super::queue::{allows_automatic, mutate, tick, CodexScope, QueueUpdate};
use super::queue_receive::reply;
use crate::session::state::SessionState;
use crate::session::test_backend::TestBackend;

#[test]
fn native_queue_read_and_failed_write_need_no_view() {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    let peer = TestBackend::default();
    let scope = CodexScope::for_test(true, false, 7);
    let mut update = QueueUpdate::default();
    assert!(!allows_automatic(&session, scope));
    assert!(tick(&peer, &mut session, scope, &mut update, false));
    let call = peer.calls.lock().expect("calls")[0].clone();
    assert_eq!(call.0, "thread/queue/list");
    assert_eq!(call.1, json!({"threadId":"root","cursor":null,"limit":1}));
    reply(
        &mut session,
        scope,
        &mut update,
        &json!({
        "requestMethod":call.0,"requestParams":call.1,"requestContext":call.2,
        "response":{"result":{"data":[],"nextCursor":null}}}),
    );
    assert!(update.select_first);
    assert!(allows_automatic(&session, scope));
    let params = json!({"threadId":"root","clientUserMessageId":"human","input":[]});
    assert!(mutate(
        &peer,
        &mut session,
        scope,
        &mut update,
        "thread/queue/add",
        params,
        Some("recover once".into())
    ));
    assert!(!allows_automatic(&session, scope));
    let call = peer.calls.lock().expect("calls")[1].clone();
    reply(
        &mut session,
        scope,
        &mut update,
        &json!({
        "requestMethod":call.0,"requestParams":{"threadId":"root","clientUserMessageId":"human"},
        "requestContext":call.2,"response":{"error":{"message":"rejected"}}}),
    );
    assert_eq!(update.recovery.as_deref(), Some("recover once"));
    assert!(!session.codex_queue().pending());
    assert!(session.take_codex_queue_recovery().is_none());
    assert_eq!(peer.calls.lock().expect("calls").len(), 2);
}
