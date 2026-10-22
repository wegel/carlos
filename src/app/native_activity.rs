//! Show Codex waits and child-agent activity without scheduling work in the client.

use serde_json::Value;

use super::tools::strip_terminal_controls;
use super::{AppState, MessageKind, Role};

/// Merge a native activity record by its item ID, including records arriving after turn completion.
pub(super) fn upsert_native_activity(app: &mut AppState, item: &Value) -> bool {
    let Some(text) = format_native_activity(item) else {
        return false;
    };
    if let Some(id) = item.get("id").and_then(Value::as_str) {
        app.upsert_mapped_message(id, Role::ToolCall, text, MessageKind::Plain, None);
    }
    true
}

fn format_native_activity(item: &Value) -> Option<String> {
    match item.get("type").and_then(Value::as_str)? {
        "sleep" => {
            let duration = item.get("durationMs").and_then(Value::as_u64)?;
            Some(format!("Wait up to {} s", duration as f64 / 1000.0))
        }
        "subAgentActivity" => format_agent_activity(item),
        "collabAgentToolCall" => Some(format_agent_call(item)),
        _ => None,
    }
}

fn format_agent_activity(item: &Value) -> Option<String> {
    let path = item.get("agentPath").and_then(Value::as_str)?;
    let action = match item.get("kind").and_then(Value::as_str)? {
        "started" => "started",
        "interacted" => "received input",
        "interrupted" => "interrupted",
        "completed" => "completed",
        _ => return None,
    };
    Some(format!("Agent {} {action}", strip_terminal_controls(path)))
}

fn format_agent_call(item: &Value) -> String {
    let action = match item.get("tool").and_then(Value::as_str).unwrap_or("") {
        "spawnAgent" => "Start agent",
        "sendInput" | "sendMessage" => "Send input to agent",
        "resumeAgent" => "Resume agent",
        "wait" => "Wait for agent",
        "closeAgent" => "Close agent",
        "followupTask" => "Send follow-up task to agent",
        "interruptAgent" => "Interrupt agent",
        "listAgents" => "List agents",
        _ => "Agent tool",
    };
    let status = item.get("status").and_then(Value::as_str).unwrap_or("");
    let status = if status == "inProgress" {
        "running"
    } else {
        status
    };
    let mut lines = vec![format!("{action}: {}", strip_terminal_controls(status))];
    if let Some(states) = item.get("agentsStates").and_then(Value::as_object) {
        for (thread_id, state) in states {
            lines.push(format_agent_state(thread_id, state));
        }
    }
    lines.join("\n")
}

fn format_agent_state(thread_id: &str, state: &Value) -> String {
    let status = state
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let mut text = format!(
        "{}: {}",
        strip_terminal_controls(thread_id),
        strip_terminal_controls(status)
    );
    if let Some(message) = state
        .get("message")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        text.push('\n');
        text.push_str(&strip_terminal_controls(message));
    }
    text
}
