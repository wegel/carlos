//! Keep original native tool results, streaming bytes and failure facts across live/reloaded paths.

use serde_json::{json, Value};

use super::{feed, session};
use crate::session::item_history::append_history_from_thread;
use crate::session::models::Role;
use crate::session::state::SessionState;

fn command_item() -> Value {
    json!({"id":"read", "type":"commandExecution", "command":"cat main.rs",
        "commandActions":[{"type":"read", "command":"cat main.rs", "path":"main.rs"}],
        "aggregatedOutput":(0..10).map(|i| format!("let source_{i} = {i};")).collect::<Vec<_>>().join("\n"), "exitCode":0})
}

fn complete(state: &mut SessionState, item: &Value) {
    feed(
        state,
        &json!({"method":"item/completed","params":{"item":item}}).to_string(),
    );
}

#[test]
fn live_and_reopened_read_outputs_show_the_same_contents_and_summary() {
    let item = command_item();
    let mut live = session("thread");
    complete(&mut live, &item);
    let mut history = session("thread");
    append_history_from_thread(
        &mut history,
        &json!({"turns":[{"status":"completed","items":[item]}]}),
    );
    assert_eq!(live.messages()[0].text, history.messages()[0].text);
    for state in [live, history] {
        assert_eq!(state.messages()[0].role, Role::ToolOutput);
        assert_eq!(state.messages()[0].file_path.as_deref(), Some("main.rs"));
        assert_eq!(
            state.messages()[0]
                .tool_output
                .as_ref()
                .expect("details")
                .summary,
            "→ Read main.rs"
        );
    }
}

#[test]
fn late_command_summaries_do_not_replace_completed_tool_contents() {
    let mut state = session("thread");
    complete(&mut state, &command_item());
    let original = state.messages()[0].text.clone();
    state.set_command_override(
        &state.backend_entry_key("read"),
        "→ Read main.rs [lines=1..10]".into(),
    );
    assert_eq!(state.messages()[0].text, original);
    assert_eq!(
        state.messages()[0]
            .tool_output
            .as_ref()
            .expect("details")
            .summary,
        "→ Read main.rs [lines=1..10]"
    );
}

#[test]
fn switching_modes_during_a_stream_keeps_every_delta_and_labels_failures() {
    let mut state = session("thread");
    let item = command_item();
    feed(
        &mut state,
        &json!({"method":"item/started","params":{"item":item}}).to_string(),
    );
    let body = item["aggregatedOutput"]
        .as_str()
        .expect("output")
        .to_string();
    for delta in [body.as_str(), "\nlast streamed line"] {
        feed(
            &mut state,
            &json!({"method":"item/commandExecution/outputDelta",
            "params":{"itemId":"read","delta":delta}})
            .to_string(),
        );
    }
    assert!(state.messages()[0].text.ends_with("last streamed line"));
    let mut failed = item;
    failed["exitCode"] = json!(2);
    failed["aggregatedOutput"] = json!(format!("{body}\ncommand failed"));
    complete(&mut state, &failed);
    assert!(state.messages()[0].text.contains("command failed"));
    assert!(
        state.messages()[0]
            .tool_output
            .as_ref()
            .expect("details")
            .failed
    );
}

#[test]
fn valid_diff_exit_one_is_not_a_failure_and_explicit_errors_are_labeled() {
    let diff = "diff --git a/main.rs b/main.rs\n--- a/main.rs\n+++ b/main.rs\n@@ -1,4 +1,4 @@\n-a\n-b\n-c\n-d\n+A\n+B\n+C\n+D";
    let mut item = json!({"id":"diff", "type":"commandExecution","command":"git diff --exit-code", "aggregatedOutput":diff, "exitCode":1});
    for error in [false, true] {
        item["isError"] = json!(error);
        let mut state = session("thread");
        complete(&mut state, &item);
        assert_eq!(
            state.messages()[0]
                .tool_output
                .as_ref()
                .expect("tool details")
                .failed,
            error
        );
    }
}

#[test]
fn completion_without_a_body_keeps_streamed_output_and_ignores_duplicate_completion() {
    let mut state = session("thread");
    let mut item = command_item();
    feed(
        &mut state,
        &json!({"method":"item/started","params":{"item":item}}).to_string(),
    );
    state.upsert_command_output_delta(
        &state.backend_entry_key("read"),
        "let streamed = 1;\nlast output line",
    );
    item["aggregatedOutput"] = Value::Null;
    complete(&mut state, &item);
    let original = state.messages()[0].text.clone();
    assert!(original.contains("last output line"));
    assert_eq!(original.matches("exit code: 0").count(), 1);
    complete(&mut state, &item);
    assert_eq!(state.messages()[0].text, original);
}
