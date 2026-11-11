//! Preserve human input and modal focus while native goals guard the Carlos automatic driver.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyModifiers, MouseEvent, MouseEventKind};
use serde_json::json;

use super::codex_goal_tests::goal;
use super::codex_question_support::{app, field_text, question, receive, Peer};
use crate::app::codex_goal_state::GoalKnowledge;
use crate::app::input_events::handle_terminal_event;
use crate::app::ralph::RalphConfig;
use crate::app::TerminalSize;
use crate::backend::PromptOrigin;

#[test]
fn an_external_driving_goal_stops_ralph_and_pending_toggles_but_retains_queued_humans() {
    let mut app = app();
    app.enable_ralph_mode(RalphConfig {
        prompt_path: PathBuf::from("fixture"),
        base_prompt: "base".into(),
        done_marker: "@@COMPLETE@@".into(),
        blocked_marker: "@@BLOCKED@@".into(),
        waiting_marker: "@@AWAITING_HUMAN@@".into(),
        continuation_prompt: "continue".into(),
    });
    app.ralph_runtime
        .enqueue_prompt("AUTOMATIC".into(), false, PromptOrigin::Automatic);
    app.ralph_runtime
        .enqueue_prompt("keep human 日本語".into(), true, PromptOrigin::Human);
    receive(
        &mut app,
        json!({"method":"thread/goal/updated","params":{"threadId":"root","goal":goal("active")}}),
    );
    assert!(!app.ralph_enabled());
    let input = app
        .dequeue_turn_input(Instant::now())
        .expect("retained queued human");
    assert_eq!(input.origin, PromptOrigin::Human);
    assert_eq!(input.text, "keep human 日本語");
    assert!(app.dequeue_turn_input(Instant::now()).is_none());
    app.codex.goals.goal = goal("paused");
    app.active_turn_id = Some("current".into());
    app.request_ralph_toggle()
        .expect("pending toggle while paused");
    receive(
        &mut app,
        json!({"method":"thread/goal/updated","params":{"threadId":"root","goal":goal("usageLimited")}}),
    );
    assert!(!app.ralph_toggle_pending());
    app.apply_pending_ralph_toggle()
        .expect("no auto-resume after native usage limit");
    assert!(!app.ralph_enabled());
}

#[test]
fn visible_question_paste_and_mouse_take_priority_over_a_hidden_goal_editor() {
    let mut app = app();
    let peer = Peer::default();
    app.codex.goals.knowledge = GoalKnowledge::Known;
    app.codex.goals.show = true;
    crate::app::codex_goals::edit(&mut app);
    app.codex
        .goals
        .editor
        .as_mut()
        .expect("goal draft")
        .objective
        .insert_str("keep goal draft");
    app.set_input_text("keep main draft");
    receive(&mut app, question(json!("form"), "root", true));
    handle_terminal_event(
        &peer,
        &mut app,
        Event::Paste("FORM_ONLY".into()),
        TerminalSize {
            width: 100,
            height: 30,
        },
    );
    assert_eq!(field_text(&app, json!("form")), "FORM_ONLY");
    assert_eq!(
        app.codex
            .goals
            .editor
            .as_ref()
            .expect("goal draft")
            .objective
            .lines()
            .join("\n"),
        "keep goal draft"
    );
    let draft = &app
        .codex
        .callbacks
        .get(&json!("form"))
        .expect("question")
        .draft;
    draft.viewport.max_scroll.set(20);
    handle_terminal_event(
        &peer,
        &mut app,
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 1,
            row: 1,
            modifiers: KeyModifiers::NONE,
        }),
        TerminalSize {
            width: 100,
            height: 30,
        },
    );
    assert_eq!(
        app.codex
            .callbacks
            .get(&json!("form"))
            .expect("question")
            .draft
            .viewport
            .scroll
            .get(),
        3
    );
    assert_eq!(app.input_text(), "keep main draft");
}

#[test]
fn a_plain_approval_without_a_form_never_pastes_into_the_hidden_goal_or_main_draft() {
    let mut app = app();
    let peer = Peer::default();
    app.codex.goals.knowledge = GoalKnowledge::Known;
    app.codex.goals.show = true;
    crate::app::codex_goals::edit(&mut app);
    receive(
        &mut app,
        json!({"id":9,"method":"item/commandExecution/requestApproval","params":{
        "threadId":"root","turnId":"t","itemId":"command","kind":"humanCommand"}}),
    );
    let pending = app.approval.pending.clone().expect("approval");
    app.codex.callbacks.entries.clear();
    app.approval.pending = Some(pending);
    app.codex.callbacks.focused = true;
    handle_terminal_event(
        &peer,
        &mut app,
        Event::Paste("MUST_NOT_EDIT".into()),
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
            .expect("goal draft")
            .objective
            .lines()
            .join("\n"),
        ""
    );
    assert!(app.input_is_empty());
}

#[test]
fn idle_wait_wakes_for_f2_refresh_but_not_for_closed_older_or_static_detail_views() {
    let mut app = app();
    let now = Instant::now();
    app.codex.goals.last_read = Some(now);
    app.codex.panel.show = true;
    app.codex.panel.refreshed = Some(now - Duration::from_millis(2950));
    assert_eq!(
        crate::app::input::idle_wait(&app, now),
        Some(Duration::from_millis(50))
    );
    for mode in 0..3 {
        app.codex.panel.show = mode != 0;
        app.codex.panel.older_window = mode == 1;
        app.codex.panel.detail = mode == 2;
        assert!(crate::app::codex_panel::wait(&app, now).is_none());
    }
    app.codex.closed = true;
    assert!(crate::app::codex_goals::wait(&app, now).is_none());
}

#[test]
fn objective_edits_send_the_observed_status_explicitly_even_if_native_was_cleared_unseen() {
    for status in [
        "paused",
        "blocked",
        "usageLimited",
        "budgetLimited",
        "complete",
    ] {
        let mut app = app();
        let peer = Peer::default();
        receive(
            &mut app,
            json!({"method":"thread/goal/updated","params":{"threadId":"root","goal":goal(status)}}),
        );
        crate::app::codex_goals::edit(&mut app);
        crate::app::codex_goals::save(&peer, &mut app);
        assert_eq!(
            peer.controls.borrow()[0].1["status"],
            status,
            "native default-active must never be used for a retained draft"
        );
    }
}

#[test]
fn clear_ack_before_notification_fences_old_reads_and_keeps_the_draft_until_new_state_is_known() {
    let mut app = app();
    let peer = Peer::default();
    receive(
        &mut app,
        json!({"method":"thread/goal/updated","params":{"threadId":"root","goal":goal("paused")}}),
    );
    crate::app::codex_goals::edit(&mut app);
    crate::app::codex_goals::tick(&peer, &mut app, Instant::now());
    app.codex.goals.confirm = Some(crate::app::codex_goal_state::GoalConfirm::Clear);
    crate::app::codex_goals::confirm(&peer, &mut app);
    super::codex_goal_tests::answer(&peer, &mut app, 1, json!({"result":{"cleared":true}}));
    assert_eq!(app.codex.goals.knowledge, GoalKnowledge::Unknown);
    crate::app::codex_goals::save(&peer, &mut app);
    assert_eq!(
        peer.controls.borrow().len(),
        2,
        "cannot save through an unresolved clear"
    );
    super::codex_goal_tests::answer(
        &peer,
        &mut app,
        0,
        json!({"result":{"goal":goal("paused")}}),
    );
    assert_eq!(app.codex.goals.knowledge, GoalKnowledge::Unknown);
    assert!(app.codex.goals.editor.is_some());
    receive(
        &mut app,
        json!({"method":"thread/goal/cleared","params":{"threadId":"root"}}),
    );
    crate::app::codex_goals::save(&peer, &mut app);
    assert_eq!(peer.controls.borrow()[2].1["status"], "paused");
    assert_eq!(peer.controls.borrow()[2].1["origin"], "user");
}
