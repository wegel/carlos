//! Keep human work queued while native goal facts guard the automatic driver.
use super::goal_contract_support::{goal, receive, session};
use crate::backend::PromptOrigin;
use crate::session::ralph::RalphConfig;
use serde_json::json;
use std::path::PathBuf;
use std::time::{Duration, Instant};
#[test]
fn an_external_driving_goal_stops_ralph_and_pending_toggles_but_retains_queued_humans() {
    let mut session = session();
    session.enable_ralph_mode(RalphConfig {
        prompt_path: PathBuf::from("fixture"),
        base_prompt: "base".into(),
        done_marker: "@@COMPLETE@@".into(),
        blocked_marker: "@@BLOCKED@@".into(),
        waiting_marker: "@@AWAITING_HUMAN@@".into(),
        continuation_prompt: "continue".into(),
    });
    session.ralph_enqueue_prompt("AUTOMATIC".into(), false, PromptOrigin::Automatic);
    session.ralph_enqueue_prompt("keep human 日本語".into(), true, PromptOrigin::Human);
    receive(
        &mut session,
        json!({"method":"thread/goal/updated","params":{"threadId":"root","goal":goal("active")}}),
    );
    assert!(!session.ralph_enabled());
    let input = session
        .ralph_runtime
        .dequeue_turn_input(Instant::now())
        .expect("retained queued human");
    assert_eq!(input.origin, PromptOrigin::Human);
    assert_eq!(input.text, "keep human 日本語");
    assert!(session
        .ralph_runtime
        .dequeue_turn_input(Instant::now())
        .is_none());
    session.set_codex_goal_for_test(goal("paused"));
    session.set_active_turn_id(Some("current".into()));
    session
        .ralph_request_toggle(true)
        .expect("pending toggle while paused");
    receive(
        &mut session,
        json!({"method":"thread/goal/updated","params":{"threadId":"root","goal":goal("usageLimited")}}),
    );
    assert!(!session.ralph_runtime().toggle_pending());
    session
        .ralph_apply_pending_toggle()
        .expect("no auto-resume after native usage limit");
    assert!(!session.ralph_enabled());
}

#[test]
fn idle_wait_wakes_for_visible_panel_refresh_but_not_closed_older_or_static_details() {
    let mut session = session();
    let now = Instant::now();
    session.set_codex_goal_read_time_for_test(now);
    session.set_codex_panel_refresh_time_for_test(now - Duration::from_millis(2950));
    let scope = super::panel_contract_support::scope(None);
    let visible = super::panel_scope::PanelScope {
        detail: false,
        ..scope
    };
    assert_eq!(
        super::panel::wait(&session, visible, now),
        Some(Duration::from_millis(50))
    );
    assert!(super::goals::wait(&session, session.codex_scope(), now, false).is_none());
    for mode in 0..3 {
        session.set_codex_panel_older_window_for_test(mode == 1);
        let scope = super::panel_scope::PanelScope {
            visible: mode != 0,
            detail: mode == 2,
            ..scope
        };
        assert!(super::panel::wait(&session, scope, now).is_none());
    }
    session.close_codex();
    assert!(super::goals::wait(&session, session.codex_scope(), now, true).is_none());
}
