//! Native goal facts and explicit actions never fabricate chat turns or compete with Ralph.

use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use serde_json::{json, Value};

use super::codex_question_support::{app, receive, Peer};
use crate::app::codex_goal_state::GoalKnowledge;
use crate::app::codex_goals::{confirm, edit, ralph_error, save, status, tick};
use crate::app::input_events::handle_terminal_event;
use crate::app::TerminalSize;

pub(super) fn goal(status: &str) -> Value {
    json!({"threadId":"root","objective":"Explicit native objective","status":status,
        "tokenBudget":1000,"tokensUsed":27,"timeUsedSeconds":3,"createdAt":100,"updatedAt":101})
}

fn observed(app: &mut crate::app::AppState, value: Value) {
    receive(
        app,
        json!({"method":"thread/goal/updated","params":{"threadId":"root","goal":value}}),
    );
}

pub(super) fn answer(peer: &Peer, app: &mut crate::app::AppState, index: usize, response: Value) {
    let (method, params) = peer.controls.borrow()[index].clone();
    receive(
        app,
        json!({"method":"carlos/codex/controlReply","params":{
        "requestMethod":method,"requestParams":params,"requestContext":peer.contexts.borrow()[index],"response":response}}),
    );
}

fn key(peer: &Peer, app: &mut crate::app::AppState, code: KeyCode, modifiers: KeyModifiers) {
    handle_terminal_event(
        peer,
        app,
        Event::Key(KeyEvent::new(code, modifiers)),
        TerminalSize {
            width: 100,
            height: 30,
        },
    );
}

#[test]
fn startup_only_reads_goal_state_and_all_six_native_statuses_remain_authoritative() {
    let mut app = app();
    let peer = Peer::default();
    assert!(ralph_error(&app).is_some());
    tick(&peer, &mut app, Instant::now());
    assert_eq!(
        peer.controls.borrow()[0],
        ("thread/goal/get".into(), json!({"threadId":"root"}))
    );
    answer(&peer, &mut app, 0, json!({"result":{"goal":null}}));
    assert!(ralph_error(&app).is_none());
    for status in [
        "active",
        "paused",
        "blocked",
        "usageLimited",
        "budgetLimited",
        "complete",
    ] {
        observed(&mut app, goal(status));
        assert_eq!(app.codex.goals.goal["status"], status);
        assert_eq!(app.codex.goals.goal["tokensUsed"], 27);
        assert_eq!(app.codex.goals.goal["timeUsedSeconds"], 3);
        assert_eq!(
            ralph_error(&app).is_none(),
            matches!(status, "paused" | "complete")
        );
    }
    assert_eq!(peer.controls.borrow().len(), 1);
}

#[test]
fn explicit_new_goal_is_paused_and_blank_budget_is_omitted_while_user_origin_is_preserved() {
    let mut app = app();
    let peer = Peer::default();
    app.codex.goals.knowledge = GoalKnowledge::Known;
    edit(&mut app);
    app.codex
        .goals
        .editor
        .as_mut()
        .expect("draft")
        .objective
        .insert_str("Build the explicit feature 日本語");
    save(&peer, &mut app);
    assert_eq!(
        peer.controls.borrow()[0],
        (
            "thread/goal/set".into(),
            json!({"threadId":"root",
        "objective":"Build the explicit feature 日本語","status":"paused","origin":"user"})
        )
    );
    assert!(app.messages.is_empty());
    assert!(app.codex.goals.editor.is_some());
}

#[test]
fn activation_requires_confirmation_and_pending_ack_blocks_ralph_until_native_result() {
    let mut app = app();
    let peer = Peer::default();
    observed(&mut app, goal("paused"));
    status(&peer, &mut app, "active");
    assert!(peer.controls.borrow().is_empty());
    confirm(&peer, &mut app);
    assert_eq!(
        peer.controls.borrow()[0].1,
        json!({"threadId":"root","status":"active","origin":"user"})
    );
    assert!(app.request_ralph_toggle().is_err());
    assert!(!app.ralph_enabled());
    assert!(ralph_error(&app).is_some());
    answer(
        &peer,
        &mut app,
        0,
        json!({"result":{"goal":goal("active")}}),
    );
    assert!(ralph_error(&app).is_some());
}

#[test]
fn invalid_updated_goal_fences_an_earlier_get_without_erasing_the_last_good_display() {
    let mut app = app();
    let peer = Peer::default();
    observed(&mut app, goal("paused"));
    tick(&peer, &mut app, Instant::now());
    for value in [
        Value::Null,
        json!({"threadId":"foreign","objective":"x","status":"paused"}),
    ] {
        observed(&mut app, value);
        assert_eq!(app.codex.goals.knowledge, GoalKnowledge::Unknown);
        assert_eq!(
            app.codex.goals.goal["objective"],
            "Explicit native objective"
        );
    }
    answer(
        &peer,
        &mut app,
        0,
        json!({"result":{"goal":goal("paused")}}),
    );
    assert_eq!(app.codex.goals.knowledge, GoalKnowledge::Unknown);
    tick(&peer, &mut app, Instant::now());
    answer(
        &peer,
        &mut app,
        1,
        json!({"result":{"goal":goal("active")}}),
    );
    assert_eq!(app.codex.goals.knowledge, GoalKnowledge::Known);
    assert!(ralph_error(&app).is_some());
}

#[test]
fn stale_or_malformed_activation_ack_cannot_release_ralph_or_delete_a_failed_draft() {
    for response in [
        json!({"result":{}}),
        json!({"result":{"goal":null}}),
        json!({"error":{"code":-32603,"message":"Unconfirmed native write"}}),
    ] {
        let mut app = app();
        let peer = Peer::default();
        observed(&mut app, goal("paused"));
        edit(&mut app);
        app.codex.goals.editor.as_mut().expect("draft").status = Some("active".into());
        save(&peer, &mut app);
        confirm(&peer, &mut app);
        answer(&peer, &mut app, 0, response);
        assert!(ralph_error(&app).is_some());
        assert_eq!(app.codex.goals.knowledge, GoalKnowledge::Unknown);
        assert!(app.codex.goals.editor.is_some());
    }
}

#[test]
fn a_paused_progress_event_before_activate_ack_requires_a_fresh_snapshot() {
    let mut app = app();
    let peer = Peer::default();
    observed(&mut app, goal("paused"));
    status(&peer, &mut app, "active");
    confirm(&peer, &mut app);
    tick(&peer, &mut app, Instant::now());
    observed(&mut app, goal("paused"));
    answer(
        &peer,
        &mut app,
        0,
        json!({"result":{"goal":goal("active")}}),
    );
    assert_eq!(app.codex.goals.knowledge, GoalKnowledge::Unknown);
    answer(
        &peer,
        &mut app,
        1,
        json!({"result":{"goal":goal("paused")}}),
    );
    assert_eq!(
        app.codex.goals.knowledge,
        GoalKnowledge::Unknown,
        "old GET cannot undo invalidation"
    );
    tick(&peer, &mut app, Instant::now());
    answer(
        &peer,
        &mut app,
        2,
        json!({"result":{"goal":goal("active")}}),
    );
    assert!(ralph_error(&app).is_some());
}

#[test]
fn external_clear_of_an_existing_draft_never_uses_natives_default_active_status() {
    let mut app = app();
    let peer = Peer::default();
    observed(&mut app, goal("paused"));
    edit(&mut app);
    assert!(app
        .codex
        .goals
        .editor
        .as_ref()
        .expect("draft")
        .status
        .is_none());
    receive(
        &mut app,
        json!({"method":"thread/goal/cleared","params":{"threadId":"root"}}),
    );
    app.ralph_runtime
        .request_toggle(true)
        .expect("pending Ralph toggle");
    save(&peer, &mut app);
    assert_eq!(peer.controls.borrow()[0].1["status"], "paused");
    assert_eq!(peer.controls.borrow()[0].1["origin"], "user");
    assert!(app.codex.goals.confirm.is_none());
}

#[test]
fn pending_goal_mutation_freezes_edits_and_paste_until_reply_then_preserves_rejected_draft() {
    let mut app = app();
    let peer = Peer::default();
    observed(&mut app, goal("paused"));
    app.codex.goals.show = true;
    edit(&mut app);
    save(&peer, &mut app);
    key(&peer, &mut app, KeyCode::Char('X'), KeyModifiers::NONE);
    crate::app::codex_goal_view::paste(
        &mut app,
        "NO_OVERWRITE",
        TerminalSize {
            width: 100,
            height: 30,
        },
    );
    assert_eq!(
        app.codex
            .goals
            .editor
            .as_ref()
            .expect("locked draft")
            .objective
            .lines()
            .join("\n"),
        "Explicit native objective"
    );
    answer(
        &peer,
        &mut app,
        0,
        json!({"error":{"code":-32600,"message":"Managed goal policy rejected this"}}),
    );
    assert!(app.codex.goals.editor.is_some());
    assert!(app
        .codex
        .goals
        .error
        .as_deref()
        .expect("visible error")
        .contains("Managed goal policy"));
    assert!(app.codex.goals.mutation.is_none());
}

#[test]
fn esc_returns_to_overview_and_pause_does_not_save_or_discard_the_independent_editor() {
    let mut app = app();
    let peer = Peer::default();
    observed(&mut app, goal("active"));
    app.codex.goals.show = true;
    edit(&mut app);
    app.codex
        .goals
        .editor
        .as_mut()
        .expect("draft")
        .objective
        .insert_str(" unsaved");
    key(&peer, &mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(app.codex.goals.show);
    assert!(!app.codex.goals.editing);
    key(&peer, &mut app, KeyCode::Char('p'), KeyModifiers::NONE);
    assert!(peer.controls.borrow()[0].1.get("objective").is_none());
    answer(
        &peer,
        &mut app,
        0,
        json!({"result":{"goal":goal("paused")}}),
    );
    assert!(app
        .codex
        .goals
        .editor
        .as_ref()
        .expect("retained draft")
        .objective
        .lines()
        .join("\n")
        .ends_with(" unsaved"));
    key(&peer, &mut app, KeyCode::Char('e'), KeyModifiers::NONE);
    assert!(app.codex.goals.editing);
}

#[test]
fn closed_and_forked_threads_cannot_accept_stale_goal_replies_or_mutations() {
    let mut app = app();
    let peer = Peer::default();
    tick(&peer, &mut app, Instant::now());
    app.set_thread_id("forked-root");
    answer(
        &peer,
        &mut app,
        0,
        json!({"result":{"goal":goal("active")}}),
    );
    assert_eq!(app.codex.goals.knowledge, GoalKnowledge::Unknown);
    assert!(app.codex.goals.goal.is_null());
    app.codex.closed = true;
    assert!(!tick(
        &peer,
        &mut app,
        Instant::now() + Duration::from_secs(40)
    ));
    assert!(ralph_error(&app).is_some());
}
