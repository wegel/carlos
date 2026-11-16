//! Preserve live rows, prompt identities and numbered changes while older native pages arrive.

use serde_json::{json, Value};

use crate::protocol_history::{HistoryCursor, HistoryPage};
use crate::session::events::Change;
use crate::session::item_history::replace_history_from_thread;
use crate::session::state::SessionState;

fn turn(id: &str, answer: &str) -> Value {
    json!({"id":id,"status":"completed","itemsView":"full","items":[
        {"type":"userMessage","id":"same-user","content":[{"type":"text","text":format!("prompt {id}")}]},
        {"type":"agentMessage","id":"same-answer","text":answer}
    ]})
}
fn cursor(next: &str) -> HistoryCursor {
    HistoryCursor {
        thread: "root".into(),
        next: next.into(),
    }
}
fn state() -> SessionState {
    let mut state = SessionState::new("root".into(), Default::default(), false);
    state.set_codex_enabled_for_test(true);
    state.enable_native_history();
    let thread = json!({"id":"root","turns":[turn("newer", "original newer answer")]});
    replace_history_from_thread(&mut state, &thread).expect("newest history");
    state.install_native_history(
        &thread,
        Some((cursor("next"), vec!["skipped-empty".into(), "next".into()])),
    );
    state.publish();
    state
}
fn page(turns: Vec<Value>, next: Option<&str>) -> HistoryPage {
    HistoryPage {
        read_bytes: 0,
        thread: json!({"id":"root","turns":turns}),
        older: next.map(cursor),
        visited: next.into_iter().map(str::to_owned).collect(),
    }
}

#[test]
fn an_older_prefix_keeps_live_text_updates_prompt_ids_and_the_existing_numbered_stream() {
    let mut state = state();
    let latest_user = state.messages()[0].id;
    let latest_answer = state.messages()[1].id;
    state.transcript_append_text(1, " LIVE", None);
    state
        .accept_older_native_history(
            &cursor("next"),
            page(vec![turn("older", "older answer")], None),
        )
        .expect("older prefix");
    assert_eq!(state.messages()[2].id, latest_user);
    assert_eq!(state.messages()[3].id, latest_answer);
    assert_eq!(state.messages()[3].text, "original newer answer LIVE");
    assert_eq!(state.input_history().history_len(), 2);
    assert_eq!(
        state.input_history().entry_id(0),
        Some(state.messages()[0].id)
    );
    assert_eq!(state.input_history().entry_id(1), Some(latest_user));
    assert_eq!(state.input_history().turn_id(latest_user), Some("newer"));
    let published = state.publish();
    assert!(!published
        .records()
        .iter()
        .any(|record| matches!(record.change, Change::Reset { .. } | Change::Added(_))));
    assert!(published.records().iter().any(
        |record| matches!(record.change, Change::Text { entry, .. } if entry == latest_answer)
    ));
    state.messages().assert_invariants();
}

#[test]
fn an_overlapping_native_turn_cannot_overwrite_a_newer_accepted_body_or_duplicate_its_prompt() {
    let mut state = state();
    let latest = state.messages()[1].id;
    let count = state
        .accept_older_native_history(
            &cursor("next"),
            page(
                vec![
                    turn("older", "older answer"),
                    turn("newer", "stale overlapping answer"),
                ],
                Some("last"),
            ),
        )
        .expect("overlap deduplicated by native turn");
    assert_eq!(count, 2);
    assert_eq!(state.messages().len(), 4);
    assert_eq!(state.messages()[3].id, latest);
    assert_eq!(state.messages()[3].text, "original newer answer");
    assert_eq!(state.input_history().history_len(), 2);
    state.messages().assert_invariants();
}

#[test]
fn repeated_skipped_cursors_and_changed_roots_preserve_the_accepted_prefix() {
    let mut state = state();
    let before = state.messages()[0].id.to_wire();
    let repeated = page(
        vec![turn("older", "must not appear")],
        Some("skipped-empty"),
    );
    assert!(state
        .accept_older_native_history(&cursor("next"), repeated)
        .is_err());
    assert_eq!(state.messages().len(), 2);
    assert_eq!(
        state
            .older_native_cursor(&before, 100)
            .expect("original cursor"),
        Some(cursor("next"))
    );
    state.set_thread_id("replacement".into());
    assert!(state
        .accept_older_native_history(
            &cursor("next"),
            page(vec![turn("older", "wrong root")], None)
        )
        .is_err());
    assert_eq!(state.messages().len(), 2);
}
