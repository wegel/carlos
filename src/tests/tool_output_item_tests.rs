//! Check that live and reopened tool results retain their contents and failure flags.

use serde_json::{json, Value};

use super::{append_history_from_thread, handle_notification_line, AppState, Role};
use crate::app::tool_output_state::ToolOutputMode;

fn command_item() -> Value {
    json!({"id":"read", "type":"commandExecution", "command":"cat main.rs",
        "commandActions":[{"type":"read", "command":"cat main.rs", "path":"main.rs"}],
        "aggregatedOutput":(0..10).map(|i| format!("let source_{i} = {i};")).collect::<Vec<_>>().join("\n"), "exitCode":0})
}

fn complete(app: &mut AppState, item: &Value) {
    handle_notification_line(
        app,
        &json!({"method":"item/completed","params":{"item":item}}).to_string(),
    );
}

#[test]
fn live_and_reopened_read_outputs_show_the_same_contents_and_summary() {
    let item = command_item();
    let mut live = AppState::new("thread".into());
    complete(&mut live, &item);
    let mut history = AppState::new("thread".into());
    append_history_from_thread(
        &mut history,
        &json!({"turns":[{"status":"completed","items":[item]}]}),
    );
    assert_eq!(live.messages[0].text, history.messages[0].text);
    for mut app in [live, history] {
        assert_eq!(app.messages[0].role, Role::ToolOutput);
        assert_eq!(app.messages[0].file_path.as_deref(), Some("main.rs"));
        app.tool_output.mode = ToolOutputMode::Compact;
        app.ensure_rendered_lines(50, None);
        assert!(app.snapshot_rendered_lines().iter().any(|line| line
            .text
            .split_whitespace()
            .last()
            .is_some_and(|n| n.parse::<usize>().is_ok())));
        app.tool_output.mode = ToolOutputMode::Full;
        app.ensure_rendered_lines(50, None);
        assert!(app
            .snapshot_rendered_lines()
            .iter()
            .any(|line| line.text.contains("source_9")));
    }
}

#[test]
fn late_command_summaries_do_not_replace_completed_tool_contents() {
    let mut app = AppState::new("thread".into());
    complete(&mut app, &command_item());
    let original = app.messages[0].text.clone();
    app.set_command_override("read", "→ Read main.rs [lines=1..10]".into());
    assert_eq!(app.messages[0].text, original);
    assert_eq!(
        app.messages[0]
            .tool_output
            .as_ref()
            .expect("details")
            .summary,
        "→ Read main.rs [lines=1..10]"
    );
}

#[test]
fn switching_modes_during_a_stream_keeps_every_delta_and_labels_failures() {
    let mut app = AppState::new("thread".into());
    let item = command_item();
    handle_notification_line(
        &mut app,
        &json!({"method":"item/started","params":{"item":item}}).to_string(),
    );
    let body = item["aggregatedOutput"]
        .as_str()
        .expect("output")
        .to_string();
    for delta in [body.as_str(), "\nlast streamed line"] {
        handle_notification_line(&mut app, &json!({"method":"item/commandExecution/outputDelta", "params":{"itemId":"read","delta":delta}}).to_string());
        app.tool_output.mode = ToolOutputMode::Compact;
        app.ensure_rendered_lines(50, None);
        assert!(app.snapshot_rendered_lines().iter().any(|line| line
            .text
            .split_whitespace()
            .last()
            .is_some_and(|n| n.parse::<usize>().is_ok())));
    }
    assert!(app.messages[0].text.ends_with("last streamed line"));
    let mut failed = item;
    failed["exitCode"] = json!(2);
    failed["aggregatedOutput"] = json!(format!("{body}\ncommand failed"));
    complete(&mut app, &failed);
    app.ensure_rendered_lines(50, None);
    let lines = app.snapshot_rendered_lines();
    assert!(lines.iter().any(|line| line.text.contains("failed")));
    assert!(lines
        .iter()
        .all(|line| !line.text.contains("command failed")));
    app.tool_output.mode = ToolOutputMode::Full;
    app.ensure_rendered_lines(50, None);
    assert!(app
        .snapshot_rendered_lines()
        .iter()
        .any(|line| line.text.contains("command failed")));
}

#[test]
fn valid_diff_exit_one_is_not_a_failure_and_explicit_errors_are_labeled() {
    let diff = "diff --git a/main.rs b/main.rs\n--- a/main.rs\n+++ b/main.rs\n@@ -1,4 +1,4 @@\n-a\n-b\n-c\n-d\n+A\n+B\n+C\n+D";
    let mut item = json!({"id":"diff", "type":"commandExecution","command":"git diff --exit-code", "aggregatedOutput":diff, "exitCode":1});
    for error in [false, true] {
        item["isError"] = json!(error);
        let mut app = AppState::new("thread".into());
        app.tool_output.mode = ToolOutputMode::Compact;
        complete(&mut app, &item);
        app.ensure_rendered_lines(80, None);
        assert_eq!(
            app.messages[0]
                .tool_output
                .as_ref()
                .expect("tool details")
                .failed,
            error
        );
        assert_eq!(
            app.snapshot_rendered_lines().iter().any(|line| line
                .text
                .split_whitespace()
                .last()
                .is_some_and(|n| n.parse::<usize>().is_ok())),
            true
        );
    }
}

#[test]
fn completion_without_a_body_keeps_streamed_output_and_ignores_duplicate_completion() {
    let mut app = AppState::new("thread".into());
    let mut item = command_item();
    handle_notification_line(
        &mut app,
        &json!({"method":"item/started","params":{"item":item}}).to_string(),
    );
    app.upsert_command_output_delta("read", "let streamed = 1;\nlast output line");
    item["aggregatedOutput"] = Value::Null;
    complete(&mut app, &item);
    let original = app.messages[0].text.clone();
    assert!(original.contains("last output line"));
    assert_eq!(original.matches("exit code: 0").count(), 1);
    complete(&mut app, &item);
    assert_eq!(app.messages[0].text, original);
}
