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
        ("read_file", ToolKind::Read),
        ("Grep", ToolKind::Search),
        ("rg", ToolKind::Search),
        ("search", ToolKind::Search),
        ("Glob", ToolKind::Search),
        ("find", ToolKind::Search),
        ("codesearch", ToolKind::Search),
        ("semeja", ToolKind::Search),
        ("Edit", ToolKind::Edit),
        ("Write", ToolKind::Edit),
        ("applypatch", ToolKind::Edit),
        ("apply_patch", ToolKind::Edit),
        ("replace", ToolKind::Edit),
        ("Bash", ToolKind::Command),
        ("shell", ToolKind::Command),
        ("exec_command", ToolKind::Command),
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

#[test]
fn native_fact_updates_preserve_source_and_can_replace_a_known_kind_with_other() {
    let mut core = session("root");
    notify(
        &mut core,
        "item/completed",
        json!({
            "id":"call", "type":"toolCall", "tool":"Opaque native title 🌍",
            "input":{"path":"é.rs"}, "kind":"read", "status":"pending",
        }),
    );
    let source = core.messages()[0].text.clone();
    let id = core.messages()[0].id;
    assert_eq!(first_tool(&mut core).status, ToolStatus::Running);
    assert!(!core.publish().snapshot(100).entries[0].done);
    for (kind, status, outcome) in [
        ("search", "in_progress", ToolStatus::Running),
        ("other", "completed", ToolStatus::Ok),
    ] {
        let mut item = json!({"id":"call", "kind":kind, "status":status});
        if status == "completed" {
            item["isError"] = json!(false);
        }
        notify(&mut core, "item/toolFacts/updated", item);
        let tool = first_tool(&mut core);
        assert_eq!(
            tool.kind,
            if kind == "search" {
                ToolKind::Search
            } else {
                ToolKind::Other
            }
        );
        assert_eq!(tool.status, outcome);
        assert_eq!(core.messages()[0].id, id);
        assert_eq!(core.messages()[0].text, source);
        assert_eq!(core.messages().len(), 1);
    }
}

#[test]
fn a_sparse_completed_command_does_not_retain_a_running_outcome_without_evidence() {
    let mut core = session("root");
    notify(
        &mut core,
        "item/started",
        json!({
            "id":"command", "type":"commandExecution", "command":"echo x", "status":"inProgress",
        }),
    );
    assert_eq!(first_tool(&mut core).status, ToolStatus::Running);
    notify(
        &mut core,
        "item/completed",
        json!({
            "id":"command", "type":"commandExecution", "aggregatedOutput":"x",
        }),
    );
    assert_eq!(first_tool(&mut core).status, ToolStatus::Unknown);
}

#[test]
fn diff_lines_that_look_like_headers_still_count_inside_the_hunk() {
    let mut core = session("root");
    notify(
        &mut core,
        "item/completed",
        json!({
            "id":"diff", "type":"toolOutput", "files":[{"filePath":"é.rs",
            "diff":"--- a/é.rs\n+++ b/é.rs\n@@ -1 +1 @@\n--- old source\n+++ new source\n--- a/second.rs\n+++ b/second.rs\n@@ -0,0 +1 @@\n+new 🌍\n"}],
        }),
    );
    let tool = first_tool(&mut core);
    assert_eq!((tool.added, tool.removed), (Some(2), Some(1)));
}

#[test]
fn a_turn_end_retires_an_unfinished_native_tool_without_inventing_success() {
    let mut core = session("root");
    core.record_turn_started("turn");
    notify(
        &mut core,
        "item/completed",
        json!({
            "id":"call", "type":"toolCall", "tool":"Read", "status":"pending",
            "input":{"path":"é.rs"}, "kind":"read",
        }),
    );
    assert_eq!(first_tool(&mut core).status, ToolStatus::Running);
    assert!(!core.publish().snapshot(100).entries[0].done);
    core.record_turn_completed("turn", crate::session::models::TurnStatus::Completed);
    let row = core.publish().snapshot(100).entries.remove(0);
    assert!(row.done);
    assert_eq!(row.tool.expect("tool").status, ToolStatus::Unknown);
}

#[test]
fn terminal_native_history_retires_stale_running_tool_facts() {
    let mut core = session("root");
    core.set_codex_enabled_for_test(true);
    core.enable_native_history();
    crate::session::item_history::replace_history_from_thread(
        &mut core,
        &json!({
            "id":"root", "turns":[{"id":"turn", "status":"completed", "items":[{
                "id":"call", "type":"toolCall", "name":"Read", "status":"pending",
                "input":{"file_path":"é.rs"},
            }]}],
        }),
    )
    .expect("checked native history");
    let row = core.publish().snapshot(100).entries.remove(0);
    assert!(row.done);
    let tool = row.tool.expect("historical native tool");
    assert_eq!(tool.kind, ToolKind::Read);
    assert_eq!(tool.status, ToolStatus::Unknown);
}

#[test]
fn split_diff_rows_summarize_their_own_source_instead_of_the_combined_output() {
    let mut core = session("root");
    let first = "@@ -1 +1 @@\n-old\n+new 🌍\n";
    let second = "@@ -0,0 +1 @@\n+another\n";
    notify(
        &mut core,
        "item/completed",
        json!({
            "id":"diff", "type":"toolOutput", "output":format!("{first}{second}"),
            "files":[{"filePath":"a.rs", "diff":first}, {"filePath":"b.rs", "diff":second}],
        }),
    );
    let snapshot = core.publish().snapshot(100);
    for (path, source, summary) in [("a.rs", first, "3 lines"), ("b.rs", second, "2 lines")] {
        let row = snapshot
            .entries
            .iter()
            .find(|row| row.file_path.as_deref() == Some(path))
            .expect("native diff part");
        assert_eq!(
            row.tool.as_ref().expect("tool").summary.as_deref(),
            Some(summary)
        );
        assert_eq!(
            core.messages()
                .entry(crate::session::entry_id::EntryId::from_wire(&row.id))
                .expect("source row")
                .text,
            source
        );
    }
}

#[test]
fn empty_diff_hunks_do_not_count_the_following_file_headers() {
    let mut core = session("root");
    notify(
        &mut core,
        "item/completed",
        json!({
            "id":"diff", "type":"toolOutput", "files":[{"filePath":"a.rs",
            "diff":"--- a/a.rs\n+++ b/a.rs\n@@ -0,0 +0,0 @@\n--- a/b.rs\n+++ b/b.rs\n@@ -0,0 +1 @@\n+new\n"}],
        }),
    );
    let tool = first_tool(&mut core);
    assert_eq!((tool.added, tool.removed), (Some(1), Some(0)));
}

#[test]
fn colored_diff_lines_keep_their_source_bytes_and_count_as_changes() {
    let mut core = session("root");
    let source = "diff --git a/é.rs b/é.rs\n--- a/é.rs\n+++ b/é.rs\n@@ -1 +1 @@\n\u{1b}[31m-old\u{1b}[0m\n\u{1b}[32m+new 🌍\u{1b}[0m\n";
    notify(
        &mut core,
        "item/completed",
        json!({
            "id":"diff", "type":"toolOutput", "files":[{"filePath":"é.rs", "diff":source}],
        }),
    );
    let tool = first_tool(&mut core);
    assert_eq!((tool.added, tool.removed), (Some(1), Some(1)));
    assert_eq!(core.messages()[0].text, source);
}
