//! Verify stale goal reads cannot overwrite current native facts without a view.

use std::time::Instant;

use serde_json::json;

use super::goal_receive::{observed, reply};
use super::goals::{can_mutate, ralph_error, tick, GoalUpdate};
use super::scope::CodexScope;
use crate::session::state::SessionState;
use crate::session::test_backend::TestBackend;

#[test]
fn stale_goal_reads_keep_newer_native_facts_and_block_ralph_without_a_view() {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    let peer = TestBackend::default();
    let scope = CodexScope {
        enabled: true,
        closed: false,
        epoch: 4,
    };
    let mut update = GoalUpdate::default();
    assert!(tick(
        &peer,
        &mut session,
        scope,
        &mut update,
        Instant::now(),
        false
    ));
    let call = peer.calls.lock().expect("calls")[0].clone();
    assert_eq!(call.0, "thread/goal/get");
    assert_eq!(call.1, json!({"threadId":"root"}));
    let active = json!({"threadId":"root","objective":"Keep native work","status":"active",
        "tokensUsed":0,"timeUsedSeconds":0,"createdAt":1,"updatedAt":2});
    observed(
        &mut session,
        &mut update,
        "thread/goal/updated",
        &json!({"threadId":"root","goal":active}),
    );
    reply(
        &mut session,
        scope,
        &mut update,
        &json!({
        "requestMethod":call.0,"requestParams":call.1,"requestContext":call.2,
        "response":{"result":{"goal":null}}}),
    );
    assert_eq!(session.codex_goals().goal(), &active);
    assert_eq!(
        ralph_error(&session, scope),
        Some("Pause or clear the Codex goal in /goal before enabling Ralph")
    );
    assert!(can_mutate(&session, scope, false).is_ok());
    session.set_codex_goal_direct_input(Some(false));
    assert_eq!(
        can_mutate(&session, scope, true),
        Err("Codex does not allow direct goal changes for this thread".into())
    );
    assert_eq!(peer.calls.lock().expect("calls").len(), 1);
}
