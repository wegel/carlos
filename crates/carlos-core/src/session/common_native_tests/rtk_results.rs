//! Keep RTK native command bytes, parsed read facts and full failure references exact.

use crate::session::models::Role;
use serde_json::{json, Value};

use super::{feed as handle_notification_line, session};
use crate::session::item_history::append_history_from_thread;
use crate::session::tools::format_tool_item;

fn read_item() -> Value {
    json!({"type":"commandExecution", "id":"rtk-read",
        "command":"rtk --verbose proxy --skip-env -- rtk read -n 'src/main file.rs'",
        "commandActions":[{"type":"unknown", "command":"rtk read -n 'src/main file.rs'"}],
        "aggregatedOutput":"1 │ fn main() {\n2 │     let value = \"hello\";\n3 │ }\n", "exitCode":0})
}

#[test]
fn full_keeps_original_rtk_commands_instead_of_the_first_parsed_action() {
    let command = "rtk proxy git status && rtk proxy cat 'src/main file.rs'";
    let item = json!({"type":"commandExecution", "command":command,
        "commandActions":[{"command":"git status"}, {"command":"cat 'src/main file.rs'"}],
        "aggregatedOutput":"output marker", "exitCode":0});
    let output = format_tool_item(&item, Role::ToolOutput).expect("formatted output");
    assert!(output.starts_with(&format!("$ {command}\n")), "{output}");
    assert!(output.contains("output marker"));
}

#[test]
fn shell_argument_arrays_keep_quoted_file_operands_and_rtk_flags() {
    let item = json!({"type":"commandExecution", "command":["rtk","proxy","cat","src/main file.rs"], "aggregatedOutput":"source", "exitCode":0});
    let output = format_tool_item(&item, Role::ToolOutput).expect("formatted output");
    assert!(
        output.starts_with("$ rtk proxy cat 'src/main file.rs'\n"),
        "{output}"
    );
}

#[test]
fn wrapped_ssh_keeps_its_original_transport_command() {
    let command = "rtk proxy ssh host 'cat main.rs'";
    let item = json!({"type":"commandExecution", "command":command, "aggregatedOutput":"source", "exitCode":0});
    assert!(format_tool_item(&item, Role::ToolOutput)
        .expect("output")
        .starts_with(&format!("$ {command}\n")));
}

#[test]
fn live_and_reopened_rtk_reads_have_the_same_path_colors_and_gutter() {
    let item = read_item();
    let mut live = session("thread");
    handle_notification_line(
        &mut live,
        &json!({"method":"item/completed", "params":{"item":item}}).to_string(),
    );
    let mut history = session("thread");
    append_history_from_thread(&mut history, &json!({"turns":[{"items":[item]}]}));
    assert_eq!(live.messages()[0].text, history.messages()[0].text);
    for app in [live, history] {
        assert_eq!(
            app.messages()[0].file_path.as_deref(),
            Some("src/main file.rs")
        );
        assert_eq!(
            app.messages()[0]
                .tool_output
                .as_ref()
                .expect("details")
                .summary,
            "→ Read src/main file.rs"
        );
    }
}

#[test]
fn rtk_failures_keep_received_details_and_saved_log_references_in_full() {
    let mut app = session("thread");
    let item = json!({"type":"commandExecution", "id":"test", "command":"rtk cargo test -p example",
        "aggregatedOutput":"FAILED: 1/3 tests\n[full output: /tmp/rtk-fixture-test.log]", "exitCode":101});
    handle_notification_line(
        &mut app,
        &json!({"method":"item/completed", "params":{"item":item}}).to_string(),
    );
    assert!(app.messages()[0]
        .text
        .contains("[full output: /tmp/rtk-fixture-test.log]"));
    assert!(
        app.messages()[0]
            .tool_output
            .as_ref()
            .expect("details")
            .failed
    );
    assert!(app.messages()[0]
        .tool_output
        .as_ref()
        .expect("details")
        .summary
        .contains("Test cargo test -p example"));
}
