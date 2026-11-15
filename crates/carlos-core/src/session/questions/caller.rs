//! Format the original native caller label without an editor or backend handle.

use serde_json::Value;

use crate::session::text::safe_label;

/// Keep the same requesting thread, MCP server or child tool pinned above a native answer field.
pub fn caller_label(method: &str, payload: &Value) -> String {
    let thread = payload["threadId"]
        .as_str()
        .or(payload["conversationId"].as_str());
    if let Some(thread) = thread.filter(|_| method != "claude/native") {
        let tail = thread
            .chars()
            .rev()
            .take(8)
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>();
        return format!(
            "Thread: …{}  {}",
            safe_label(&tail),
            safe_label(payload["serverName"].as_str().unwrap_or("Codex question"))
        );
    }
    if let Some(server) = payload["serverName"].as_str() {
        return format!("MCP server: {}", safe_label(server));
    }
    let mut label = format!(
        "Tool: {}",
        safe_label(payload["toolName"].as_str().unwrap_or("unknown"))
    );
    if let Some(agent) = payload["agentID"].as_str() {
        label.push_str(&format!("  Child: {}", safe_label(agent)));
    }
    label
}
