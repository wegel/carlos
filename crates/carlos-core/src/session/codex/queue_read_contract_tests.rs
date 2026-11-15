//! Fence native queue pages by root, epoch, revision and the original snapshot deadline.

use std::time::Duration;

use serde_json::{json, Value};

use super::queue::{allows_automatic, page, tick, CodexScope, QueueUpdate};
use super::queue_contract_support::{call, entry, reply, session};
use super::queue_receive;
use crate::session::test_backend::TestBackend;

#[test]
fn stale_and_old_epoch_pages_cannot_authorize_automatic_work_or_replace_a_complete_snapshot() {
    let mut state = session();
    let peer = TestBackend::default();
    let scope = state.codex_scope();
    let mut update = QueueUpdate::default();
    assert!(!allows_automatic(&state, scope));
    assert!(tick(&peer, &mut state, scope, &mut update, false));
    state.invalidate_codex_queue();
    queue_receive::reply(
        &mut state,
        scope,
        &mut update,
        &reply(&peer, 0, json!({"result":{"data":[],"nextCursor":null}})),
    );
    assert!(!state.codex_queue().known());
    assert!(!state.codex_queue().reading());
    assert!(tick(&peer, &mut state, scope, &mut update, false));
    let old = reply(
        &peer,
        1,
        json!({"result":{"data":[entry("old", "old queued")],"nextCursor":null}}),
    );
    let new_epoch = CodexScope::for_test(true, false, scope.epoch + 1);
    queue_receive::reply(&mut state, new_epoch, &mut update, &old);
    assert!(state.codex_queue().entries().is_empty());
    assert!(
        state.codex_queue().reading(),
        "an old epoch cannot retire the current read"
    );
    state.adopt_native_thread("forked".into());
    queue_receive::reply(&mut state, scope, &mut update, &old);
    assert!(state.codex_queue().entries().is_empty());
    assert!(tick(&peer, &mut state, scope, &mut update, false));
    assert_eq!(
        call(&peer, 2).1,
        json!({"threadId":"forked","cursor":null,"limit":1})
    );
    queue_receive::reply(
        &mut state,
        scope,
        &mut update,
        &reply(&peer, 2, json!({"result":{"data":[],"nextCursor":null}})),
    );
    assert!(allows_automatic(&state, scope));
    state.set_codex_queue_entries_for_test(vec![entry("human", "human first")]);
    assert!(!allows_automatic(&state, scope));
}

#[test]
fn invalid_queue_pages_keep_automatic_work_blocked_instead_of_installing_partial_history() {
    let mut state = session();
    let peer = TestBackend::default();
    let scope = state.codex_scope();
    let mut update = QueueUpdate::default();
    assert!(tick(&peer, &mut state, scope, &mut update, false));
    assert_eq!(call(&peer, 0).0, "thread/queue/list");
    assert_eq!(
        call(&peer, 0).1,
        json!({"threadId":"root","cursor":null,"limit":1})
    );
    let next = queue_receive::reply(
        &mut state,
        scope,
        &mut update,
        &reply(
            &peer,
            0,
            json!({"result":{"data":[entry("a", "first")],"nextCursor":"next"}}),
        ),
    );
    assert!(!state.codex_queue().known());
    let (token, cursor) = next.expect("continuation uses the captured snapshot token");
    assert_eq!(call(&peer, 0).2["token"], token);
    page(&peer, &mut state, scope, &mut update, token, cursor);
    assert_eq!(
        call(&peer, 1).1,
        json!({"threadId":"root","cursor":"next","limit":1})
    );
    assert_eq!(call(&peer, 1).2["epoch"], call(&peer, 0).2["epoch"]);
    queue_receive::reply(
        &mut state,
        scope,
        &mut update,
        &reply(
            &peer,
            1,
            json!({"result":{"data":[entry("a", "first")],"nextCursor":"next"}}),
        ),
    );
    assert!(!allows_automatic(&state, scope));
    assert!(state.codex_queue().entries().is_empty());
    assert!(!state.codex_queue().reading());
    assert!(state
        .codex_queue()
        .error()
        .expect("read error")
        .contains("changed across pages"));
}

#[test]
fn later_queue_pages_use_the_remaining_whole_snapshot_deadline() {
    let mut state = session();
    let peer = TestBackend::default();
    let scope = state.codex_scope();
    let mut update = QueueUpdate::default();
    assert!(tick(&peer, &mut state, scope, &mut update, false));
    let token = call(&peer, 0).2["token"].as_u64().expect("snapshot token");
    state.age_codex_queue_read_for_test(Duration::from_secs(19));
    page(
        &peer,
        &mut state,
        scope,
        &mut update,
        token,
        json!("next-page"),
    );
    assert!(
        call(&peer, 1).2["timeoutMs"]
            .as_u64()
            .expect("remaining milliseconds")
            <= 1000
    );
    assert_eq!(
        call(&peer, 1).1,
        json!({"threadId":"root","cursor":"next-page","limit":1})
    );
    state.age_codex_queue_read_for_test(Duration::from_secs(21));
    page(
        &peer,
        &mut state,
        scope,
        &mut update,
        token,
        json!("last-page"),
    );
    assert!(!state.codex_queue().reading());
    assert_eq!(
        peer.calls.lock().expect("calls").len(),
        2,
        "expired snapshot sends no further page"
    );
    assert!(state
        .codex_queue()
        .error()
        .expect("deadline error")
        .contains("20 second deadline"));
}

#[test]
fn repeated_cursors_and_oversized_pages_never_publish_their_partial_entries() {
    let cases = [
        json!({"data":[],"nextCursor":"same"}),
        json!({"data":(0..4097).map(|i| entry(&format!("item-{i}"), "text")).collect::<Vec<_>>(),"nextCursor":null}),
        json!({"data":[entry("huge", &"x".repeat(16 * 1024 * 1024))],"nextCursor":null}),
    ];
    for result in cases {
        let mut state = session();
        let peer = TestBackend::default();
        let scope = state.codex_scope();
        let mut update = QueueUpdate::default();
        assert!(tick(&peer, &mut state, scope, &mut update, false));
        let token = call(&peer, 0).2["token"].as_u64().expect("snapshot token");
        if result["nextCursor"] == "same" {
            assert!(queue_receive::reply(
                &mut state,
                scope,
                &mut update,
                &reply(&peer, 0, json!({"result":result}))
            )
            .is_some());
            page(&peer, &mut state, scope, &mut update, token, json!("same"));
        }
        queue_receive::reply(
            &mut state,
            scope,
            &mut update,
            &reply(&peer, 0, json!({"result":result})),
        );
        assert!(state.codex_queue().entries().is_empty());
        assert!(!state.codex_queue().known());
        assert!(!state.codex_queue().reading());
        assert!(state.codex_queue().error().is_some());
        assert!(!allows_automatic(&state, scope));
    }
}

#[test]
fn queue_read_guards_send_no_request_until_the_native_snapshot_needs_one() {
    let peer = TestBackend::default();
    let mut state = session();
    let mut update = QueueUpdate::default();
    for (scope, blocked) in [
        (CodexScope::for_test(false, false, 1), false),
        (CodexScope::for_test(true, true, 1), false),
        (CodexScope::for_test(true, false, 1), true),
    ] {
        assert!(!tick(&peer, &mut state, scope, &mut update, blocked));
    }
    assert!(peer.calls.lock().expect("calls").is_empty());
    let scope = state.codex_scope();
    assert!(tick(&peer, &mut state, scope, &mut update, false));
    assert!(!tick(&peer, &mut state, scope, &mut update, false));
    queue_receive::reply(
        &mut state,
        scope,
        &mut update,
        &reply(&peer, 0, json!({"result":{"data":[],"nextCursor":null}})),
    );
    assert!(!tick(&peer, &mut state, scope, &mut update, false));
    assert_eq!(peer.calls.lock().expect("calls").len(), 1);
    state.invalidate_codex_queue();
    assert!(tick(&peer, &mut state, scope, &mut update, false));
    queue_receive::reply(
        &mut state,
        scope,
        &mut update,
        &reply(&peer, 1, json!({"error":{"message":"read unavailable"}})),
    );
    assert!(!tick(&peer, &mut state, scope, &mut update, false));
    assert_eq!(peer.calls.lock().expect("calls").len(), 2);
    assert_eq!(state.codex_queue().error(), Some("read unavailable"));
    assert_eq!(Value::Null, call(&peer, 1).1["cursor"]);
}
