//! Keep current text, pending prompts and native scope intact while an older archive arrives.

use serde_json::{json, Value};

use crate::claude_backend::history_window::{HistoryCursor, HistoryPage};
use crate::session::events::Change;
use crate::session::models::{Role, TurnStatus};
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
    HistoryPage {
        lines,
        older: None,
        read_bytes: 0,
    }
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
fn an_older_call_refreshes_its_result_already_in_the_live_window() {
    let mut state = state();
    state.receive_claude_message("item/completed", &json!({"history":true,
        "sourceKey":{"sessionId":"root","kind":"tool","id":"tool","part":"result"},
        "item":{"id":"tool:result","type":"toolResult","tool":"Bash","command":"pwd","output":"original output"}}));
    let before = state.publish().snapshot(100).entries.remove(1);
    assert!(before.tool.expect("native result").call.is_none());
    let line = json!({"method":"item/completed","params":{"history":true,
        "sourceKey":{"sessionId":"root","kind":"tool","id":"tool","part":"call"},
        "item":{"id":"tool","type":"toolCall","name":"Bash","input":{"command":"pwd"}}}})
    .to_string();
    state
        .accept_older_claude_history(
            &cursor(),
            HistoryPage {
                lines: vec![line],
                older: None,
                read_bytes: 0,
            },
        )
        .expect("older call source");
    let published = state.publish();
    assert!(published
        .records()
        .iter()
        .any(|record| matches!(record.change,
        Change::Facts(id) if id.to_wire() == before.id)));
    let rows = published.snapshot(100).entries;
    assert_eq!(
        rows[2].tool.as_ref().expect("result").call.as_ref(),
        Some(&rows[0].id)
    );
    assert!(!published
        .records()
        .iter()
        .any(|record| matches!(record.change, Change::Reset { .. } | Change::Added(_))));
}

#[test]
fn complete_pairs_in_a_large_older_page_do_not_publish_live_changes_or_overflow() {
    let mut state = state();
    let mut lines = Vec::new();
    for index in 0..300 {
        let id = format!("older-{index}");
        for (part, item) in [
            (
                "call",
                json!({"id":id,"type":"toolCall","name":"Bash","input":{"command":"pwd"}}),
            ),
            (
                "result",
                json!({"id":format!("{id}:result"),"type":"toolResult","tool":"Bash","command":"pwd","output":"original output"}),
            ),
        ] {
            lines.push(
                json!({"method":"item/completed","params":{"history":true,
                "sourceKey":{"sessionId":"root","kind":"tool","id":id,"part":part},"item":item}})
                .to_string(),
            );
        }
    }
    state
        .accept_older_claude_history(
            &cursor(),
            HistoryPage {
                lines,
                older: None,
                read_bytes: 0,
            },
        )
        .expect("complete older pairs");
    let published = state.publish();
    assert!(!published.records().iter().any(|record| matches!(
        record.change,
        Change::Facts(_) | Change::Reset { .. } | Change::Added(_)
    )));
    assert_eq!(published.snapshot(1000).entries.len(), 601);
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

#[test]
fn archive_adoption_keeps_live_bookkeeping_and_carries_only_historical_row_facts() {
    let mut state = state();
    state.set_active_turn_id(Some("live-turn".into()));
    state.record_historical_turn_status("live-turn", TurnStatus::InProgress);
    let key = state.backend_entry_key("live-command");
    state.set_command_override(&key, "newer command label".into());
    state.mark_item_completed(&state.backend_entry_key("completed-live-item"));
    state
        .update_claude_task(&json!({"subtype":"task_started","task_id":"live-task","run_id":"run"}))
        .expect("current native task");
    let statuses = state.turn_status_by_id.clone();
    let overrides = state.command_render_overrides.clone();
    let completed = state.completed_item_ids.clone();
    let tasks = serde_json::to_value(state.export_tasks()).expect("task facts");
    state
        .accept_older_claude_history(&cursor(), page())
        .expect("archive prefix");
    assert_eq!(state.active_turn_id().as_deref(), Some("live-turn"));
    assert_eq!(state.turn_status_by_id, statuses);
    assert_eq!(state.command_render_overrides, overrides);
    assert_eq!(
        state.completed_item_ids, completed,
        "archive keys do not replace live guards"
    );
    assert_eq!(
        serde_json::to_value(state.export_tasks()).expect("task facts"),
        tasks
    );
    assert!(
        state.messages().is_complete(state.messages()[1].id),
        "completed archive answer is carried with its row"
    );
    assert!(
        !state.messages().is_complete(state.messages()[2].id),
        "live answer remains active"
    );
}
