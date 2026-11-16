//! A temporarily unavailable native handle must not strand a prepared optional context read.

use std::sync::atomic::Ordering;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use crate::engine::owner::{Channels, Owner};
use crate::session::actions::ServerRequestAction;
use crate::session::context_usage::ContextUsage;
use crate::session::state::SessionState;
use crate::session::test_backend::TestBackend;

struct Fixture(Owner);

impl Drop for Fixture {
    fn drop(&mut self) {
        self.0.channels.stop.store(true, Ordering::SeqCst);
        self.0.native.stop();
        self.0.workers.stop();
    }
}

fn fixture() -> Fixture {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.enable_claude();
    let channels = Channels {
        stop_request: Default::default(),
        stopping: Default::default(),
        ordinary: mpsc::channel().1,
        control: mpsc::channel().1,
        reads: mpsc::channel().1,
        wake: mpsc::sync_channel(1).0,
        wake_rx: mpsc::channel().1,
        notice: mpsc::sync_channel(1).0,
        stop: Arc::default(),
        changed: Arc::default(),
        goal_interest: Arc::default(),
        task_interest: Arc::default(),
    };
    Fixture(Owner::new(
        session,
        Box::<TestBackend>::default(),
        mpsc::channel().1.into(),
        channels,
    ))
}

#[test]
fn unavailable_command_capture_releases_the_context_slot_without_erasing_known_counts() {
    for known in [
        None,
        Some(ContextUsage {
            used: 32000,
            max: 123000,
        }),
    ] {
        let mut fixture = fixture();
        let owner = &mut fixture.0;
        owner.session.set_context_usage(known);
        let now = Instant::now();
        let params = owner
            .session
            .prepare_claude_context(now)
            .1
            .expect("prepared optional read");
        owner.action(ServerRequestAction::ClaudeControl {
            method: "context".into(),
            params,
        });
        // TestBackend has no captured command handle: exercise the real pre-dispatch failure.
        owner.dispatch_actions();
        assert_eq!(owner.session.context_usage(), known);
        assert!(!owner.session.events.backend_closed);
        assert_eq!(owner.workers.actions_busy, 0);
        assert!(
            owner
                .session
                .prepare_claude_context(now + Duration::from_secs(4))
                .1
                .is_none(),
            "keep the existing failure backoff"
        );
        assert!(
            owner
                .session
                .prepare_claude_context(now + Duration::from_secs(6))
                .1
                .is_some(),
            "a capture failure cannot leave an unsent context read pending forever"
        );
    }
}
