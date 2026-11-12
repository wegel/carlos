//! Enumerate turn boundaries against the former saved-offset rules.

use super::models::{Role, TurnStatus};
use super::ralph::RalphConfig;
use super::state::SessionState;

#[test]
fn every_short_turn_history_keeps_the_same_suffix_and_first_entry() {
    let mut checked = 0;
    for code in 0..7usize.pow(5) {
        let mut session = SessionState::new("thread".into());
        let mut old_start = None;
        let mut first = None;
        let mut choices = code;
        for _ in 0..5 {
            apply_step(&mut session, choices % 7, &mut old_start, &mut first);
            choices /= 7;
            assert_eq!(session.turn_start(), old_start);
            assert_eq!(session.turn_boundary.as_ref().and_then(|b| b.first), first);
            let start = old_start.unwrap_or(session.messages.len());
            assert_eq!(
                session.messages.iter().skip(start).count(),
                session.messages.len().saturating_sub(start)
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 84_035);
}

fn apply_step(
    session: &mut SessionState,
    step: usize,
    old_start: &mut Option<usize>,
    first: &mut Option<super::entry_id::EntryId>,
) {
    match step {
        0 => {
            session.mark_turn_started();
            *old_start = Some(session.messages.len());
            *first = None;
        }
        1..=3 => {
            let (index, _) = session.append_message(
                if step == 1 {
                    Role::Assistant
                } else {
                    Role::System
                },
                "entry",
            );
            if *old_start == Some(index) {
                *first = Some(session.messages[index].id);
            }
        }
        4 => {
            session.handle_ralph_turn_completed(false);
            *old_start = None;
            *first = None;
        }
        5 => {
            session.handle_ralph_turn_completed(true);
            *old_start = None;
            *first = None;
        }
        _ => {
            session.reset_for_forked_thread();
            *old_start = None;
            *first = None;
        }
    }
}

#[test]
fn empty_and_interrupted_turns_scan_only_the_saved_ralph_suffix() {
    for prefix in ["plain", "@@COMPLETE@@", "@@BLOCKED@@", "@@AWAITING_HUMAN@@"] {
        for suffix in [
            None,
            Some("plain"),
            Some("@@COMPLETE@@"),
            Some("@@BLOCKED@@"),
            Some("@@AWAITING_HUMAN@@"),
        ] {
            for interrupted in [false, true] {
                for record_start in [false, true] {
                    check_markers(prefix, suffix, interrupted, record_start);
                }
            }
        }
    }
}

fn check_markers(prefix: &str, suffix: Option<&str>, interrupted: bool, record_start: bool) {
    let mut session = SessionState::new("thread".into());
    session.enable_ralph_mode(config());
    session.append_message(Role::Assistant, prefix);
    if record_start {
        assert!(session.record_turn_started("turn"));
    }
    if let Some(suffix) = suffix {
        session.append_message(Role::Assistant, suffix);
    }
    if interrupted {
        session.append_turn_interrupted_marker();
    }
    session.record_turn_completed(
        "turn",
        if interrupted {
            TurnStatus::Interrupted
        } else {
            TurnStatus::Completed
        },
    );
    session.handle_ralph_turn_completed(interrupted);
    let hit = record_start && suffix.is_some_and(|s| s.starts_with("@@"));
    assert_eq!(session.ralph_enabled(), !hit);
    assert_eq!(
        session.has_pending_ralph_continuation(),
        !hit && !interrupted
    );
    assert_eq!(session.turn_start(), None);
}

fn config() -> RalphConfig {
    RalphConfig {
        prompt_path: "prompt".into(),
        base_prompt: "start".into(),
        done_marker: "@@COMPLETE@@".into(),
        blocked_marker: "@@BLOCKED@@".into(),
        waiting_marker: "@@AWAITING_HUMAN@@".into(),
        continuation_prompt: "continue".into(),
    }
}
