//! A paused or closed native reader cannot keep the owner awake on an overdue quota deadline.

use std::time::{Duration, Instant};

use super::rate_limits_wait;
use crate::session::state::SessionState;
use crate::session::test_backend::TestBackend;

#[test]
fn a_closed_native_reader_has_no_quota_wakeup_even_when_its_deadline_is_overdue() {
    let now = Instant::now();
    let session = due_session(now);
    assert_eq!(rate_limits_wait(&session, false, now), Some(Duration::ZERO));
    assert_eq!(rate_limits_wait(&session, true, now), None);
}

#[test]
fn rebuilding_and_closed_codex_scopes_pause_quota_wakeups_until_reads_are_ready() {
    let now = Instant::now();
    let mut session = due_session(now);
    session.codex_connection.rebuilding = true;
    assert_eq!(rate_limits_wait(&session, false, now), None);
    session.codex_connection.rebuilding = false;
    assert_eq!(rate_limits_wait(&session, false, now), Some(Duration::ZERO));
    session.close_codex();
    assert_eq!(rate_limits_wait(&session, false, now), None);
}

fn due_session(now: Instant) -> SessionState {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.enable_codex();
    session.tick_rate_limits(&TestBackend::default(), now - Duration::from_secs(301));
    session
}
