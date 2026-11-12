//! Enumerate context correlation and quota deadlines without real backend processes.

use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::app::claude_context::{receive, tick, wait};
use crate::app::state::AppState;
use crate::backend::BackendKind;
use crate::session::claude::ClaudePhase;
use crate::session::context_usage::ContextUsage;

fn app() -> AppState {
    let mut app = AppState::new("root".into());
    app.session.set_claude_enabled_for_test(true);
    app
}

#[test]
fn context_token_and_revision_guards_settle_only_the_owned_read() {
    for (request, expected, retry) in [
        (json!({"contextToken":1,"revision":0}), Some(17), false),
        (json!({"contextToken":2,"revision":0}), None, false),
        (json!({"contextToken":1,"revision":99}), None, true),
        (json!({"revision":0}), Some(17), false),
        (
            json!({"contextToken":"bad","revision":"bad"}),
            Some(17),
            false,
        ),
    ] {
        let mut app = app();
        let peer = QuotaPeer::default();
        let now = Instant::now();
        assert!(!tick(&peer, &mut app, now));
        assert_eq!(
            peer.calls.borrow()[0],
            ("context".into(), json!({"revision":0,"contextToken":1}))
        );
        receive(
            &mut app,
            &json!({"ok":true,"result":{"totalTokens":17,"rawMaxTokens":100}}),
            &request,
        );
        assert_eq!(
            app.session.context_usage().map(|usage| usage.used),
            expected,
            "{request}"
        );
        tick(&peer, &mut app, now);
        assert_eq!(peer.calls.borrow().len(), if retry { 2 } else { 1 });
    }
}

#[test]
fn context_reply_defaults_keep_known_counts_when_the_reply_cannot_supply_valid_counts() {
    for (ok, result, expected, fresh) in [
        (
            true,
            json!({"totalTokens":7,"rawMaxTokens":100,"maxTokens":200}),
            (7, 100),
            true,
        ),
        (
            true,
            json!({"totalTokens":7,"maxTokens":200}),
            (7, 200),
            true,
        ),
        (
            true,
            json!({"totalTokens":7,"rawMaxTokens":0,"maxTokens":200}),
            (42, 1000),
            false,
        ),
        (
            true,
            json!({"totalTokens":-1,"maxTokens":200}),
            (42, 1000),
            false,
        ),
        (true, json!({"maxTokens":200}), (42, 1000), false),
        (
            false,
            json!({"totalTokens":7,"maxTokens":200}),
            (42, 1000),
            false,
        ),
    ] {
        let mut app = app();
        app.session.set_context_usage(Some(ContextUsage {
            used: 42,
            max: 1000,
        }));
        receive(&mut app, &json!({"ok":ok,"result":result}), &Value::Null);
        let usage = app
            .session
            .context_usage()
            .expect("last known counts remain");
        assert_eq!((usage.used, usage.max), expected);
        assert_eq!(app.session.claude().context_fresh(), fresh);
    }
}

#[test]
fn unavailable_context_readers_never_send_or_wake() {
    for (enabled, phase, label) in [
        (false, ClaudePhase::Ready, None),
        (true, ClaudePhase::Starting, None),
        (true, ClaudePhase::Closed, None),
        (true, ClaudePhase::Ready, Some("Compacting")),
    ] {
        let mut app = app();
        app.session.set_claude_enabled_for_test(enabled);
        app.session.set_claude_phase_for_test(phase);
        app.session.record_claude_status(&json!({"label":label}));
        let peer = QuotaPeer::default();
        let now = Instant::now();
        assert!(!tick(&peer, &mut app, now));
        assert!(wait(&app, now).is_none());
        assert!(peer.calls.borrow().is_empty());
    }
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
    assert_eq!(
        peer.calls.borrow()[0],
        ("usage.optional".into(), json!({"revision":0}))
    );
    assert_eq!(
        peer.calls.borrow()[1],
        ("usage.optional".into(), json!({"revision":0}))
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

impl crate::backend::BackendClient for QuotaPeer {
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
    fn take_events_rx(&mut self) -> anyhow::Result<crate::backend::EventReceiver> {
        unreachable!()
    }
    fn stop(&mut self) {}
}
