//! Preserve native goal status, revision and root guards without terminal state.
use super::goal_contract_support::{
    activate, answer, calls, goal, mutate, observed, ralph_error, receive, session, tick,
};
use super::goal_state::GoalKnowledge;
use crate::session::test_backend::TestBackend;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

#[test]
fn startup_only_reads_goal_state_and_all_six_native_statuses_remain_authoritative() {
    let mut session = session();
    let peer = TestBackend::default();
    assert!(ralph_error(&session).is_some());
    tick(&peer, &mut session, Instant::now());
    assert_eq!(
        calls(&peer)[0],
        ("thread/goal/get".into(), json!({"threadId":"root"}))
    );
    answer(&peer, &mut session, 0, json!({"result":{"goal":null}}));
    assert!(ralph_error(&session).is_none());
    for status in [
        "active",
        "paused",
        "blocked",
        "usageLimited",
        "budgetLimited",
        "complete",
    ] {
        observed(&mut session, goal(status));
        assert_eq!(session.codex_goals().goal()["status"], status);
        assert_eq!(session.codex_goals().goal()["tokensUsed"], 27);
        assert_eq!(session.codex_goals().goal()["timeUsedSeconds"], 3);
        assert_eq!(
            ralph_error(&session).is_none(),
            matches!(status, "paused" | "complete")
        );
    }
    assert_eq!(calls(&peer).len(), 1);
}

#[test]
fn invalid_updated_goal_fences_an_earlier_get_without_erasing_the_last_good_display() {
    let mut session = session();
    let peer = TestBackend::default();
    observed(&mut session, goal("paused"));
    tick(&peer, &mut session, Instant::now());
    for value in [
        Value::Null,
        json!({"threadId":"foreign","objective":"x","status":"paused"}),
    ] {
        observed(&mut session, value);
        assert_eq!(session.codex_goals().knowledge(), GoalKnowledge::Unknown);
        assert_eq!(
            session.codex_goals().goal()["objective"],
            "Explicit native objective"
        );
    }
    answer(
        &peer,
        &mut session,
        0,
        json!({"result":{"goal":goal("paused")}}),
    );
    assert_eq!(session.codex_goals().knowledge(), GoalKnowledge::Unknown);
    tick(&peer, &mut session, Instant::now());
    answer(
        &peer,
        &mut session,
        1,
        json!({"result":{"goal":goal("active")}}),
    );
    assert_eq!(session.codex_goals().knowledge(), GoalKnowledge::Known);
    assert!(ralph_error(&session).is_some());
}

#[test]
fn stale_or_malformed_activation_ack_keeps_native_knowledge_unknown_and_blocks_ralph() {
    for response in [
        json!({"result":{}}),
        json!({"result":{"goal":null}}),
        json!({"error":{"code":-32603,"message":"Unconfirmed native write"}}),
    ] {
        let mut session = session();
        let peer = TestBackend::default();
        observed(&mut session, goal("paused"));
        activate(&peer, &mut session);
        assert!(ralph_error(&session).is_some());
        assert!(super::goals::can_mutate(&session, session.codex_scope(), false).is_err());
        answer(&peer, &mut session, 0, response);
        assert!(ralph_error(&session).is_some());
        assert_eq!(session.codex_goals().knowledge(), GoalKnowledge::Unknown);
    }
}

#[test]
fn a_paused_progress_event_before_activate_ack_requires_a_fresh_snapshot() {
    let mut session = session();
    let peer = TestBackend::default();
    observed(&mut session, goal("paused"));
    activate(&peer, &mut session);
    tick(&peer, &mut session, Instant::now());
    observed(&mut session, goal("paused"));
    answer(
        &peer,
        &mut session,
        0,
        json!({"result":{"goal":goal("active")}}),
    );
    assert_eq!(session.codex_goals().knowledge(), GoalKnowledge::Unknown);
    answer(
        &peer,
        &mut session,
        1,
        json!({"result":{"goal":goal("paused")}}),
    );
    assert_eq!(
        session.codex_goals().knowledge(),
        GoalKnowledge::Unknown,
        "old GET cannot undo invalidation"
    );
    tick(&peer, &mut session, Instant::now());
    answer(
        &peer,
        &mut session,
        2,
        json!({"result":{"goal":goal("active")}}),
    );
    assert!(ralph_error(&session).is_some());
}

#[test]
fn closed_and_forked_threads_cannot_accept_stale_goal_replies_or_mutations() {
    let mut session = session();
    let peer = TestBackend::default();
    tick(&peer, &mut session, Instant::now());
    session.adopt_native_thread("forked-root".into());
    answer(
        &peer,
        &mut session,
        0,
        json!({"result":{"goal":goal("active")}}),
    );
    assert_eq!(session.codex_goals().knowledge(), GoalKnowledge::Unknown);
    assert!(session.codex_goals().goal().is_null());
    session.close_codex();
    assert!(!tick(
        &peer,
        &mut session,
        Instant::now() + Duration::from_secs(40)
    ));
    assert!(ralph_error(&session).is_some());
}

#[test]
fn clear_ack_before_notification_fences_old_reads_and_blocks_mutations_until_fresh_state() {
    let mut session = session();
    let peer = TestBackend::default();
    observed(&mut session, goal("paused"));
    tick(&peer, &mut session, Instant::now());
    mutate(
        &peer,
        &mut session,
        "thread/goal/clear",
        json!({"threadId":"root","origin":"user"}),
    );
    answer(&peer, &mut session, 1, json!({"result":{"cleared":true}}));
    assert_eq!(session.codex_goals().knowledge(), GoalKnowledge::Unknown);
    assert!(super::goals::can_mutate(&session, session.codex_scope(), false).is_err());
    assert_eq!(calls(&peer).len(), 2);
    answer(
        &peer,
        &mut session,
        0,
        json!({"result":{"goal":goal("paused")}}),
    );
    assert_eq!(session.codex_goals().knowledge(), GoalKnowledge::Unknown);
    receive(
        &mut session,
        json!({"method":"thread/goal/cleared","params":{"threadId":"root"}}),
    );
    assert!(super::goals::can_mutate(&session, session.codex_scope(), false).is_ok());
    assert!(session.codex_goals().goal().is_null());
}
