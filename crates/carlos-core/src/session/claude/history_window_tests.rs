//! Keep current text, pending prompts and native scope intact while an older archive arrives.

use serde_json::{json, Value};

use crate::claude_backend::history_window::{HistoryCursor, HistoryPage};
use crate::session::events::Change;
use crate::session::models::Role;
use crate::session::state::SessionState;

fn state() -> SessionState {
    let mut state = SessionState::new("root".into(), Default::default(), false);
    state.enable_claude();
    let row = state.append_message(Role::Assistant, "newer answer");
    state.messages.activate(state.messages()[row].id);
    state.input_history.record_history("pending human", None);
    state
        .install_claude_history_window(&metadata())
        .expect("newest archive window");
    state.publish();
    state
}

fn metadata() -> Value {
    json!({"windowStart":2,"windowEnd":4,"olderOffset":2,"nextOffset":null,"snapshotId":"snapshot"})
}

fn cursor() -> HistoryCursor {
    HistoryCursor {
        thread: "root".into(),
        snapshot: "snapshot".into(),
        end: 2,
    }
}

fn page() -> HistoryPage {
    let lines = vec![
        json!({"method":"claude/historyRecord","params":{"record":{"uuid":"older-user","session_id":"root","opaque":true}}}),
        json!({"method":"claude/historyPrompt","params":{"source":"human","uuid":"older-user","text":"older human"}}),
        json!({"method":"item/completed","params":{"history":true,"sourceKey":{"sessionId":"root","kind":"assistant","id":"older-answer","block":0},
            "item":{"type":"agentMessage","id":"claude-msg-0-1-0","text":"older answer"}}}),
    ].into_iter().map(|line|line.to_string()).collect();
    HistoryPage { lines, older: None }
}

#[test]
fn older_claude_rows_preserve_live_text_pending_prompt_and_numbered_changes() {
    let mut state = state();
    let latest = state.messages()[0].id;
    state.transcript_append_text(0, " LIVE", None);
    assert_eq!(
        state
            .accept_older_claude_history(&cursor(), page())
            .expect("older prefix"),
        2
    );
    assert_eq!(state.messages()[2].id, latest);
    assert_eq!(state.messages()[2].text, "newer answer LIVE");
    assert!(
        !state.messages().is_complete(latest),
        "the live suffix stays active"
    );
    assert_eq!(state.input_history().text(0), "older human");
    assert_eq!(
        state.input_history().turn_id(state.messages()[0].id),
        Some("older-user")
    );
    assert_eq!(state.input_history().text(1), "pending human");
    assert_eq!(state.input_history().entry_id(1), None);
    assert!(state
        .older_claude_history_cursor()
        .expect("exhausted cursor")
        .is_none());
    let published = state.publish();
    assert!(!published
        .records()
        .iter()
        .any(|row| matches!(row.change, Change::Reset { .. } | Change::Added(_))));
    assert!(published
        .records()
        .iter()
        .any(|row| matches!(row.change,Change::Text{entry,..} if entry==latest)));
    state.messages().assert_invariants();
}

#[test]
fn replaced_helpers_and_changed_roots_cannot_adopt_an_old_claude_window() {
    let mut state = state();
    state.claude.generation += 1;
    assert!(state
        .accept_older_claude_history(&cursor(), page())
        .is_err());
    assert_eq!(state.messages().len(), 1);
    state.set_thread_id("replacement".into());
    state.set_thread_id("root".into());
    assert!(state
        .older_claude_history_cursor()
        .expect("retired archive")
        .is_none());
    assert!(state
        .accept_older_claude_history(&cursor(), page())
        .is_err());
    assert_eq!(state.messages()[0].text, "newer answer");
}

#[test]
fn invalid_newest_metadata_preserves_the_original_older_cursor() {
    let mut state = state();
    for (field, value) in [
        ("olderOffset", json!("bad")),
        ("nextOffset", json!("bad")),
        ("windowStart", json!(5)),
        ("snapshotId", json!(7)),
        ("windowEnd", json!(-1)),
    ] {
        let mut invalid = metadata();
        invalid[field] = value;
        assert!(
            state.install_claude_history_window(&invalid).is_err(),
            "invalid {field} accepted"
        );
        assert_eq!(
            state
                .older_claude_history_cursor()
                .expect("original cursor"),
            Some(cursor())
        );
    }
}
