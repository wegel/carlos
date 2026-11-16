//! Check the compact phone facts through real native events and history adoption.

use carlos_proto::session::{LabelSource, Tool, ToolKind, ToolStatus};
use serde_json::{json, Value};

use super::{feed, session};
use crate::session::state::SessionState;

fn notify(core: &mut SessionState, method: &str, item: Value) {
    feed(
        core,
        &json!({"method":method, "params":{
            "threadId":"root", "turnId":"turn", "item":item,
        }})
        .to_string(),
    );
}

fn first_tool(core: &mut SessionState) -> Tool {
    core.publish()
        .snapshot(100)
        .entries
        .remove(0)
        .tool
        .expect("native tool")
}

#[test]
fn native_outcomes_distinguish_observed_success_failure_interruption_and_unknown() {
    for (facts, expected) in [
        (json!({"exitCode":0}), ToolStatus::Ok),
        (json!({"exitCode":3}), ToolStatus::Failed),
        (json!({"isError":false}), ToolStatus::Ok),
        (json!({"isError":true}), ToolStatus::Failed),
        (
            json!({"status":"interrupted", "exitCode":1}),
            ToolStatus::Interrupted,
        ),
        (
            json!({"interrupted":true, "isError":true}),
            ToolStatus::Interrupted,
        ),
        (json!({"status":"completed"}), ToolStatus::Unknown),
    ] {
        let mut core = session("root");
        let mut item = json!({"id":"tool", "type":"commandExecution", "command":"printf x", "aggregatedOutput":"x"});
        item.as_object_mut()
            .expect("item")
            .extend(facts.as_object().expect("facts").clone());
        notify(&mut core, "item/completed", item);
        let tool = first_tool(&mut core);
        assert_eq!((tool.kind, tool.status), (ToolKind::Command, expected));
        assert_eq!(tool.summary.as_deref(), Some("1 line"));
    }
}

#[test]
fn a_generated_label_does_not_change_native_kind_outcome_or_source() {
    let mut core = session("root");
    notify(
        &mut core,
        "item/completed",
        json!({
            "id":"tool", "type":"toolResult", "tool":"Grep", "isError":false, "output":"é\n🌍\n",
        }),
    );
    let before = core.messages()[0].text.clone();
    let id = core.messages()[0].id;
    let label = "Read a label 🌍".repeat(300);
    core.messages.set_label(id, label.clone(), true);
    let tool = first_tool(&mut core);
    assert_eq!((tool.kind, tool.status), (ToolKind::Search, ToolStatus::Ok));
    assert_eq!(tool.label_source, LabelSource::Ai);
    assert_eq!(tool.label, label);
    assert_eq!(tool.summary.as_deref(), Some("2 lines"));
    assert_eq!(core.messages()[0].text, before);
}

#[test]
fn diff_counts_keep_known_zero_and_native_diff_exit_one_is_success() {
    for (diff, counts) in [
        ("diff --git a/é.rs b/é.rs\nsimilarity index 100%\n", (0, 0)),
        ("--- a/é.rs\n+++ b/é.rs\n@@ -1 +1 @@\n-old\n+🌍\n", (1, 1)),
    ] {
        let mut core = session("root");
        notify(
            &mut core,
            "item/completed",
            json!({
                "id":"tool", "type":"toolOutput", "files":[{"filePath":"é.rs", "diff":diff}],
            }),
        );
        let tool = first_tool(&mut core);
        assert_eq!(tool.kind, ToolKind::Edit);
        assert_eq!((tool.added, tool.removed), (Some(counts.0), Some(counts.1)));
        assert_eq!(tool.status, ToolStatus::Unknown);
    }
    let mut core = session("root");
    notify(
        &mut core,
        "item/completed",
        json!({
            "id":"diff", "type":"commandExecution", "command":"git diff", "exitCode":1,
            "aggregatedOutput":"diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-old\n+new\n",
        }),
    );
    assert_eq!(first_tool(&mut core).status, ToolStatus::Ok);
}

#[test]
fn tool_calls_keep_their_native_kind_before_a_result_exists() {
    for (name, kind) in [
        ("Read", ToolKind::Read),
        ("Grep", ToolKind::Search),
        ("Edit", ToolKind::Edit),
    ] {
        let mut core = session("root");
        notify(
            &mut core,
            "item/started",
            json!({
                "id":"call", "type":"toolCall", "tool":name, "input":{"file_path":"é.rs"},
            }),
        );
        let tool = first_tool(&mut core);
        assert_eq!((tool.kind, tool.status), (kind, ToolStatus::Running));
        assert_eq!(tool.summary, None);
    }
}
