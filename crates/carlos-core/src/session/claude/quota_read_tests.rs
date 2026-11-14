//! Reject optional quota replies from replaced roots and preserve the native five-minute deadline.

use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::QuotaReadKey;
use crate::session::state::SessionState;
use crate::session::usage::unix_now;

fn session() -> SessionState {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.enable_claude();
    session
}

fn reply(session: &mut SessionState, context: &Value, used: u64) {
    session.receive_claude_quota_report(
        &json!({"ok":true,"result":{"windows":[{"key":"five_hour","label":"Five hours",
            "usedPercent":used,"resetsAt":unix_now()+3600}]}}),
        context,
    );
}

#[test]
fn optional_quota_failure_retires_only_its_request_without_retrying_before_five_minutes() {
    let mut session = session();
    let now = Instant::now();
    let first = session.prepare_claude_quota(now).expect("initial read");
    let key = QuotaReadKey::from_context(&first).expect("read identity");
    session.fail_claude_quota_request(key);
    assert!(!session.claude_quota_request_current(key));
    assert!(session
        .prepare_claude_quota(now + Duration::from_secs(299))
        .is_none());
    let next = session
        .prepare_claude_quota(now + Duration::from_secs(300))
        .expect("deadline read");
    let next_key = QuotaReadKey::from_context(&next).expect("new identity");
    session.fail_claude_quota_request(key);
    assert!(session.claude_quota_request_current(next_key));
    reply(&mut session, &first, 90);
    assert!(session.claude().quota().windows().is_empty());
    reply(&mut session, &next, 20);
    assert_eq!(session.claude().quota().windows()["five_hour"].used(), 20.0);
}

#[test]
fn optional_quota_cannot_cross_a_native_helper_or_conversation_scope() {
    let mut session = session();
    let now = Instant::now();
    let old = session.prepare_claude_quota(now).expect("initial read");
    session.set_claude_generation_for_test(2);
    assert!(!session.claude_quota_request_current(QuotaReadKey::from_context(&old).expect("key")));
    reply(&mut session, &old, 90);
    assert!(session.claude().quota().windows().is_empty());
    let next = session
        .prepare_claude_quota(now + Duration::from_secs(300))
        .expect("new helper read");
    reply(&mut session, &old, 90);
    reply(&mut session, &next, 30);
    assert_eq!(session.claude().quota().windows()["five_hour"].used(), 30.0);
}

#[test]
fn a_newer_native_stream_window_survives_an_older_optional_reply() {
    let mut session = session();
    let old = session
        .prepare_claude_quota(Instant::now())
        .expect("initial read");
    session.claude_quota_event(&json!({"rateLimitType":"five_hour", "utilization":0.45,
        "resetsAt":unix_now()+3600}));
    reply(&mut session, &old, 10);
    assert_eq!(session.claude().quota().windows()["five_hour"].used(), 45.0);
    let current = session
        .prepare_claude_quota(Instant::now() + Duration::from_secs(301))
        .expect("later read");
    session.receive_claude_quota_report(
        &json!({"ok":false,"error":{"message":"unavailable"}}),
        &current,
    );
    assert_eq!(session.claude().quota().windows()["five_hour"].used(), 45.0);
}
