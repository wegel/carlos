//! Identifies private child workstreams from native message metadata.

use serde_json::Value;

/// Keep child-owned conversation records outside the main transcript.
pub(crate) fn is_child_message(message: &Value) -> bool {
    message["agent_id"].is_string()
        || !message["parent_tool_use_id"].is_null()
        || !message["parent_agent_id"].is_null()
        || message["isSidechain"] == true
}

/// Hide raw in-process task reports while retaining the main assistant's own report.
pub(crate) fn is_child_report_notice(message: &Value) -> bool {
    if message["type"] != "user" {
        return false;
    }
    let origin = &message["origin"];
    match origin["kind"].as_str() {
        Some("task-notification") => origin["subkind"].is_null() && nonempty(&origin["runId"]),
        Some("peer" | "observer") => nonempty(&origin["senderTaskId"]),
        _ => false,
    }
}

fn nonempty(value: &Value) -> bool {
    value.as_str().is_some_and(|text| !text.is_empty())
}
