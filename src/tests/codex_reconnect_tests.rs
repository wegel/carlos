//! Retain the quota-ready boundary until its native socket successor lands.

use std::time::Instant;

use super::codex_reconnect_support::{app, ready, snapshot, Peer};

#[test]
fn due_quota_does_not_write_or_spin_before_ready_then_refreshes_once() {
    let peer = Peer::default();
    let mut app = app();
    let now = Instant::now();
    app.tick_rate_limits(&peer, now - std::time::Duration::from_secs(400));
    peer.native.controls.borrow_mut().clear();
    assert!(!crate::app::input::tick_rate_limits(&peer, &mut app, now));
    assert!(crate::app::input::idle_wait(&app, now).is_none());
    assert!(peer.native.controls.borrow().is_empty());
    ready(&peer, &mut app, snapshot("fresh", "idle"));
    app.session.set_codex_runtime_refresh_for_test(false);
    assert!(crate::app::input::tick_rate_limits(&peer, &mut app, now));
    crate::app::input::tick_rate_limits(&peer, &mut app, now);
    assert_eq!(peer.native.controls.borrow().len(), 1);
    assert_eq!(
        peer.native.controls.borrow()[0].0,
        "account/rateLimits/read"
    );
    assert!(
        crate::app::input::idle_wait(&app, now).expect("future quota timer")
            > std::time::Duration::ZERO
    );
}
