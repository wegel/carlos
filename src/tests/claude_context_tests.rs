//! Keep root context readable through native work, late reads and session boundaries.

use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::codex_question_support::Peer;
use super::{AppState, ContextUsage};
use crate::app::claude_context::{invalidate, receive, refresh, tick, wait};
use crate::app::notifications::handle_server_message_line;

fn app() -> AppState {
    let mut app = AppState::new("root".into());
    app.claude.enabled = true;
    app
}

fn sdk(app: &mut AppState, message: Value) {
    let _ = handle_server_message_line(
        app,
        &json!({"method":"claude/controller","params":{
        "frame":{"type":"event","event":"sdk","message":message}}})
        .to_string(),
    );
}

fn answer(app: &mut AppState, context: &Value, used: u64) {
    receive(
        app,
        &json!({"ok":true,"result":{"totalTokens":used,"maxTokens":90000,"rawMaxTokens":100000}}),
        context,
    );
}

#[test]
fn repeated_running_and_child_usage_keep_known_root_context_and_one_pending_read() {
    let mut app = app();
    let peer = Peer::default();
    let now = Instant::now();
    tick(&peer, &mut app, now);
    let request = peer.controls.borrow()[0].1.clone();
    answer(&mut app, &request, 42000);
    let revision = app.claude.activity.context_revision;
    for _ in 0..3 {
        sdk(
            &mut app,
            json!({"type":"system","subtype":"session_state_changed","state":"running"}),
        );
        tick(&peer, &mut app, now);
    }
    assert_eq!(app.claude.activity.context_revision, revision);
    assert_eq!(
        app.context_usage,
        Some(ContextUsage {
            used: 42000,
            max: 100000
        })
    );
    assert_eq!(peer.controls.borrow().len(), 1);
    tick(&peer, &mut app, now + Duration::from_secs(2));
    assert_eq!(peer.controls.borrow().len(), 2);
    sdk(
        &mut app,
        json!({"type":"assistant","agent_id":"child","message":{"usage":{"input_tokens":999999},"content":[]}}),
    );
    assert_eq!(app.context_usage.expect("root context").used, 42000);
}

#[test]
fn stalled_read_marks_known_counts_stale_without_more_requests_and_recovers_when_it_settles() {
    let mut app = app();
    let peer = Peer::default();
    let now = Instant::now();
    tick(&peer, &mut app, now);
    let first = peer.controls.borrow()[0].1.clone();
    answer(&mut app, &first, 42000);
    assert!(
        wait(&app, now + Duration::from_secs(60)).is_none(),
        "idle snapshots do not poll forever"
    );
    sdk(
        &mut app,
        json!({"type":"system","subtype":"session_state_changed","state":"running"}),
    );
    tick(&peer, &mut app, now + Duration::from_secs(2));
    let request = peer.controls.borrow()[1].1.clone();
    app.claude.context.fresh = true;
    assert!(tick(&peer, &mut app, now + Duration::from_secs(8)));
    assert!(!app.claude.context.fresh);
    assert!(wait(&app, now + Duration::from_secs(8)).is_none());
    tick(&peer, &mut app, now + Duration::from_secs(60));
    assert_eq!(peer.controls.borrow().len(), 2);
    answer(&mut app, &request, 43000);
    assert!(app.claude.context.fresh);
    assert_eq!(
        app.context_usage.expect("new snapshot replaces old").used,
        43000
    );
    tick(&peer, &mut app, now + Duration::from_secs(61));
    assert_eq!(peer.controls.borrow().len(), 3);
}

#[test]
fn obsolete_compaction_and_model_reads_release_the_slot_without_restoring_old_counts() {
    let mut app = app();
    let peer = Peer::default();
    let now = Instant::now();
    tick(&peer, &mut app, now);
    let before = peer.controls.borrow()[0].1.clone();
    sdk(
        &mut app,
        json!({"type":"system","subtype":"status","status":"compacting"}),
    );
    answer(&mut app, &before, 99000);
    assert!(app.context_usage.is_none());
    sdk(
        &mut app,
        json!({"type":"system","subtype":"status","status":null,"compact_result":"success"}),
    );
    tick(&peer, &mut app, now);
    let fresh = peer.controls.borrow().last().expect("new read").1.clone();
    answer(&mut app, &fresh, 10000);
    assert_eq!(app.context_usage.expect("post-compact count").used, 10000);
    app.claude.observed_model = Some("old".into());
    refresh(&mut app);
    tick(&peer, &mut app, now + Duration::from_secs(2));
    let old_model = peer.controls.borrow().last().expect("model read").1.clone();
    sdk(
        &mut app,
        json!({"type":"system","subtype":"init","model":"new"}),
    );
    answer(&mut app, &old_model, 80000);
    assert!(
        app.context_usage.is_none(),
        "same-batch old-model reply is fenced before tick"
    );
    tick(&peer, &mut app, now);
    assert_eq!(peer.controls.borrow().len(), 4);
}

#[test]
fn optional_read_failure_keeps_known_counts_and_retries_without_failing_chat() {
    let mut app = app();
    let peer = Peer::default();
    let now = Instant::now();
    tick(&peer, &mut app, now);
    let first = peer.controls.borrow()[0].1.clone();
    answer(&mut app, &first, 42000);
    refresh(&mut app);
    tick(&peer, &mut app, now + Duration::from_secs(2));
    let request = peer.controls.borrow()[1].1.clone();
    receive(
        &mut app,
        &json!({"ok":false,"error":{"message":"optional unavailable"}}),
        &request,
    );
    assert_eq!(app.context_usage.expect("last known retained").used, 42000);
    assert!(!app.claude.context.fresh);
    assert!(app.messages.is_empty());
    tick(&peer, &mut app, now);
    assert_eq!(peer.controls.borrow().len(), 2);
    tick(&peer, &mut app, now + Duration::from_secs(8));
    assert_eq!(peer.controls.borrow().len(), 3);
    invalidate(&mut app);
    assert!(app.context_usage.is_none());
}

#[test]
fn helper_replacement_retires_pending_read_and_old_generation_cannot_install_branch_counts() {
    let mut app = app();
    let peer = Peer::default();
    tick(&peer, &mut app, Instant::now());
    let old = peer.controls.borrow()[0].1.clone();
    let _ = handle_server_message_line(
        &mut app,
        &json!({"method":"claude/sessionChanged","params":{"generation":2,"sessionId":"branch"}})
            .to_string(),
    );
    let _ = handle_server_message_line(&mut app, &json!({"method":"claude/controller","params":{"generation":1,"requestMethod":"context","requestContext":old,
        "frame":{"type":"reply","ok":true,"result":{"totalTokens":999,"rawMaxTokens":1000}}}}).to_string());
    assert!(app.context_usage.is_none());
    app.claude.phase = crate::app::claude_state::ClaudePhase::Ready;
    tick(&peer, &mut app, Instant::now());
    assert_eq!(peer.controls.borrow().len(), 2);
}

#[test]
fn phone_status_distinguishes_stale_counts_then_redraws_fresh_native_values() {
    let mut app = app();
    app.context_usage = Some(ContextUsage {
        used: 42000,
        max: 100000,
    });
    for width in [36, 18] {
        let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, 12))
            .expect("terminal");
        terminal
            .draw(|frame| crate::app::render::render_main_view(frame, &mut app))
            .expect("stale draw");
        let stale: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(stale.contains('~'));
        if width == 36 {
            assert!(stale.contains("42k/100k"));
        }
        answer(&mut app, &Value::Null, 43000);
        terminal
            .draw(|frame| crate::app::render::render_main_view(frame, &mut app))
            .expect("fresh draw");
        let fresh: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(!fresh.contains('~'));
        app.claude.context.fresh = false;
    }
}

struct FailedWriter;
impl crate::backend::BackendClient for FailedWriter {
    fn kind(&self) -> crate::backend::BackendKind {
        crate::backend::BackendKind::Claude
    }
    fn call(&self, _: &str, _: Value, _: Duration) -> anyhow::Result<String> {
        unreachable!()
    }
    fn respond(&self, _: &Value, _: Value) -> anyhow::Result<()> {
        unreachable!()
    }
    fn take_events_rx(&mut self) -> anyhow::Result<std::sync::mpsc::Receiver<String>> {
        unreachable!()
    }
    fn request_native_control(&self, _: &str, _: Value) -> anyhow::Result<()> {
        anyhow::bail!("context write unavailable")
    }
    fn respond_error(&self, _: &Value, _: i64, _: &str) -> anyhow::Result<()> {
        unreachable!()
    }
    fn stop(&mut self) {}
}

#[test]
fn immediate_write_failure_and_tick_only_model_change_request_a_visible_redraw() {
    let mut app = app();
    app.context_usage = Some(ContextUsage {
        used: 42000,
        max: 100000,
    });
    app.claude.context.fresh = true;
    app.record_turn_started("busy");
    assert!(tick(&FailedWriter, &mut app, Instant::now()));
    assert!(!app.claude.context.fresh);
    assert!(app.context_usage.is_some());
    app.runtime.current_model = Some("changed".into());
    assert!(tick(&FailedWriter, &mut app, Instant::now()));
    assert!(app.context_usage.is_none());
}
