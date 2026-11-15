//! Enumerate context correlation and quota deadlines without real backend processes.

use std::time::{Duration, Instant};

use carlos_core::backend::BackendKind;
use carlos_core::session::claude::ClaudePhase;
use serde_json::{json, Value};

use crate::app::state::AppState;

fn app() -> AppState {
    let mut app = AppState::new("root".into());
    app.session.set_claude_enabled_for_test(true);
    app
}

#[test]
fn quota_optional_reads_and_countdown_draws_keep_their_independent_deadlines() {
    let mut app = app();
    check_quota_deadlines(&mut app);
}

fn check_quota_deadlines(app: &mut AppState) {
    let peer = QuotaPeer {
        kind: BackendKind::Claude,
        fail: true,
        calls: std::cell::RefCell::new(Vec::new()),
    };
    let now = Instant::now();
    for (seconds, redraw, reads, wait_seconds) in [
        (0, true, 1, 60),
        (59, false, 1, 1),
        (60, true, 1, 60),
        (299, true, 1, 1),
        (300, false, 2, 59),
        (359, true, 2, 60),
    ] {
        assert_eq!(
            crate::app::claude_quota::tick(app, &peer, now + Duration::from_secs(seconds)),
            redraw
        );
        assert_eq!(peer.calls.borrow().len(), reads);
        assert_eq!(
            crate::app::claude_quota::wait(app, now + Duration::from_secs(seconds)),
            Some(Duration::from_secs(wait_seconds))
        );
    }
    let calls = peer.calls.borrow();
    let scope = calls[0].1["quotaScope"]
        .as_str()
        .expect("original native scope");
    assert_eq!(scope.len(), 64);
    assert_eq!(
        calls[0],
        (
            "usage.optional".into(),
            json!({"revision":0,"quotaRead":1,"quotaScope":scope})
        )
    );
    assert_eq!(
        calls[1],
        (
            "usage.optional".into(),
            json!({"revision":0,"quotaRead":2,"quotaScope":scope})
        )
    );
    assert!(
        app.session.messages().is_empty(),
        "optional write failure stays silent"
    );
}

#[test]
fn quota_timer_guards_skip_nonclaude_and_unready_helpers() {
    for (kind, phase) in [
        (BackendKind::Codex, ClaudePhase::Ready),
        (BackendKind::Grok, ClaudePhase::Ready),
        (BackendKind::Claude, ClaudePhase::Starting),
        (BackendKind::Claude, ClaudePhase::Closed),
    ] {
        let mut app = app();
        app.session.set_claude_phase_for_test(phase);
        let peer = QuotaPeer {
            kind,
            fail: false,
            calls: std::cell::RefCell::new(Vec::new()),
        };
        let now = Instant::now();
        assert!(!crate::app::claude_quota::tick(&mut app, &peer, now));
        assert!(crate::app::claude_quota::wait(&app, now).is_none());
        assert!(peer.calls.borrow().is_empty());
    }
}

struct QuotaPeer {
    kind: BackendKind,
    fail: bool,
    calls: std::cell::RefCell<Vec<(String, Value)>>,
}

impl Default for QuotaPeer {
    fn default() -> Self {
        Self {
            kind: BackendKind::Claude,
            fail: false,
            calls: std::cell::RefCell::new(Vec::new()),
        }
    }
}

impl carlos_core::backend::BackendClient for QuotaPeer {
    fn kind(&self) -> BackendKind {
        self.kind
    }
    fn request_native_control(&self, method: &str, params: Value) -> anyhow::Result<()> {
        self.calls.borrow_mut().push((method.into(), params));
        anyhow::ensure!(!self.fail, "fixed optional-write failure");
        Ok(())
    }
    fn call(&self, _: &str, _: Value, _: Duration) -> anyhow::Result<String> {
        unreachable!()
    }
    fn respond(&self, _: &Value, _: Value) -> anyhow::Result<()> {
        unreachable!()
    }
    fn respond_error(&self, _: &Value, _: i64, _: &str) -> anyhow::Result<()> {
        unreachable!()
    }
    fn take_events_rx(&mut self) -> anyhow::Result<carlos_core::backend::EventReceiver> {
        unreachable!()
    }
    fn stop(&mut self) {}
}
