//! Check turn suffixes against hand-written histories and Ralph outcomes.

use super::history_identity::{HistoryEntry, HistoryMode};
use super::models::{EntryDraft, Role, TurnStatus};
use super::state::SessionState;
use crate::session::ralph::RalphConfig;

#[test]
fn turns_keep_the_expected_transcript_suffix() {
    for (case, expected) in [
        ("empty", vec![]),
        ("interrupted empty", vec!["Turn interrupted"]),
        ("index zero", vec!["fresh"]),
        ("rewind", vec!["fresh"]),
        ("history replacement", vec!["fresh"]),
    ] {
        let session = turn_case(case);
        let start = session.turn_start().expect("started turn");
        let suffix: Vec<_> = session
            .messages
            .iter()
            .skip(start)
            .map(|entry| entry.text.as_str())
            .collect();
        assert_eq!(suffix, expected, "{case}");
    }
}

fn turn_case(case: &str) -> SessionState {
    let mut session = SessionState::new("thread".into(), Default::default(), false);
    session.enable_ralph_mode(config());
    if case != "index zero" {
        session.append_message(Role::Assistant, "@@COMPLETE@@");
    }
    if matches!(case, "rewind" | "history replacement") {
        session.append_message(Role::Assistant, "old prefix");
        session.record_turn_started("old turn");
        session.append_message(Role::Assistant, "discarded");
        session.record_turn_completed("old turn", TurnStatus::Completed);
        if case == "rewind" {
            session.truncate_at(1);
        } else {
            session.replace_history(
                vec![HistoryEntry {
                    draft: EntryDraft::plain(Role::Assistant, "@@BLOCKED@@"),
                    key: None,
                    occurrences: 1,
                }],
                HistoryMode::SameThread,
            );
        }
    }
    session.record_turn_started("turn");
    match case {
        "empty" => {}
        "interrupted empty" => {
            session.append_turn_interrupted_marker();
        }
        _ => {
            session.append_message(Role::Assistant, "fresh");
        }
    }
    session
}

#[test]
fn empty_and_interrupted_turns_scan_only_the_saved_ralph_suffix() {
    for (case, appended, interrupted, enabled, continuation) in [
        ("empty", None, false, true, true),
        ("interrupted empty", None, true, true, false),
        ("index zero", None, false, true, true),
        ("rewind", None, false, true, true),
        ("history replacement", None, false, true, true),
        ("empty", Some("@@COMPLETE@@"), false, false, false),
        ("empty", Some("@@BLOCKED@@"), false, false, false),
        ("empty", Some("@@AWAITING_HUMAN@@"), false, false, false),
        ("empty", Some("@@COMPLETE@@"), true, false, false),
        ("empty", Some("plain"), true, true, false),
    ] {
        let mut session = turn_case(case);
        if let Some(text) = appended {
            session.append_message(Role::Assistant, text);
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
        assert_eq!(session.ralph_enabled(), enabled, "{case} {appended:?}");
        assert_eq!(
            session.has_pending_ralph_continuation(),
            continuation,
            "{case} {appended:?}"
        );
        assert_eq!(session.turn_start(), None);
    }
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
