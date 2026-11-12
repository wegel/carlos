//! Shows native SDK permission requests without inventing follow-up user prompts.

use serde_json::{json, Value};

use super::claude_question_state::{safe_label, safe_text, NativeDraft};
use super::state::{AppState, ApprovalRequestDraft, ApprovalRequestKind};
use crate::session::approval_state::ApprovalQueue;

/// Show or queue one native SDK permission or question by its request ID.
pub(super) fn receive_callback(app: &mut AppState, frame: &Value) {
    let Some(id) = frame["callbackId"].as_str() else {
        return;
    };
    let payload = &frame["payload"];
    let kind = frame["kind"].as_str().unwrap_or("");
    let draft = NativeDraft::from_payload(kind, payload);
    let Some(prompt) = callback_prompt(id, kind, payload) else {
        app.append_message(
            super::Role::System,
            format!(
                "Unsupported Claude callback {}: {}",
                safe_text(kind),
                safe_text(id)
            ),
        );
        return;
    };
    if app
        .session
        .approval()
        .pending()
        .is_some_and(|pending| pending.facts().request_id == prompt.request_id)
        || app
            .session
            .approval()
            .queued(ApprovalQueue::Claude)
            .iter()
            .any(|pending| pending.facts().request_id == prompt.request_id)
    {
        return;
    }
    let has_fields = !draft.fields.is_empty();
    app.claude.answers.drafts.insert(id.to_string(), draft);
    super::claude_browser::register(app, id, payload);
    if app.session.approval().pending().is_none() {
        app.set_pending_approval(prompt);
    } else {
        app.session.enqueue_approval(prompt, ApprovalQueue::Claude);
    }
    let entry = app
        .live_approval_cards()
        .find(|p| p.facts().request_id == id)
        .map(|p| p.entry_id())
        .expect("session-issued callback id");
    app.view.approval_hints.register_callback(entry, has_fields);
}

fn callback_prompt(id: &str, kind: &str, payload: &Value) -> Option<ApprovalRequestDraft> {
    let (title, request_kind) = match kind {
        "permission" => (
            payload["title"]
                .as_str()
                .or(payload["displayName"].as_str())
                .unwrap_or("Allow Claude tool?"),
            ApprovalRequestKind::ClaudeNative,
        ),
        "elicitation" => (
            payload["title"]
                .as_str()
                .or(payload["message"].as_str())
                .unwrap_or("Claude needs an answer"),
            ApprovalRequestKind::ClaudeQuestion,
        ),
        _ => return None,
    };
    Some(ApprovalRequestDraft {
        request_id: json!(id),
        method: "claude/native".to_string(),
        kind: request_kind,
        title: safe_label(title),
        detail_lines: callback_details(payload, kind),
        requested_permissions: Some(payload.clone()),
        can_accept_for_session: false,
        can_decline: true,
        can_cancel: true,
    })
}

/// Withdraw exactly the native callback that the SDK cancelled.
pub(super) fn cancel_callback(app: &mut AppState, id: &Value) {
    if let Some(id) = id.as_str() {
        app.claude.answers.drafts.remove(id);
        super::claude_browser::withdraw(app, id);
    }
    if app.view.confirm_allow.is_some_and(|entry| {
        app.live_approval_cards()
            .any(|pending| pending.entry_id() == entry && &pending.facts().request_id == id)
    }) {
        app.view.confirm_allow = None;
    }
    let retired = app
        .session
        .retire_queued_approvals(id, ApprovalQueue::Claude);
    app.drop_retired_approval_hints(retired);
    if app
        .session
        .approval()
        .pending()
        .is_some_and(|pending| &pending.facts().request_id == id)
    {
        app.clear_pending_approval();
    }
}

fn callback_details(payload: &Value, kind: &str) -> Vec<String> {
    let mut lines = Vec::new();
    for (key, label) in [
        ("toolName", "Tool"),
        ("displayName", "Action"),
        ("description", "Description"),
        ("decisionReason", "Reason"),
        ("agentID", "Child agent"),
        ("toolUseID", "Tool call"),
        ("blockedPath", "Blocked path"),
        ("serverName", "MCP server"),
        ("message", "Message"),
    ] {
        if let Some(text) = payload[key].as_str().filter(|text| !text.is_empty()) {
            let text = if matches!(key, "description" | "decisionReason" | "message") {
                safe_text(text)
            } else {
                safe_label(text)
            };
            lines.push(format!("{label}: {text}"));
        }
    }
    push_server_details(payload, &mut lines);
    push_tool_details(payload, kind, &mut lines);
    if kind == "elicitation" {
        lines.push("Server config source: unknown".to_string());
    }
    if let Some(url) = payload["url"].as_str() {
        lines.insert(
            0,
            format!(
                "URL host: {}",
                safe_label(payload["urlHost"].as_str().unwrap_or("unavailable"))
            ),
        );
        lines.insert(1, format!("Link: {}", safe_label(url)));
    }
    lines
}

fn push_server_details(payload: &Value, lines: &mut Vec<String>) {
    if let Some(server) = payload["mcpServer"].as_object() {
        lines.push(format!(
            "MCP server: {} (source: {})",
            safe_label(
                server
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            ),
            safe_label(
                server
                    .get("source")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            )
        ));
    }
}

fn push_tool_details(payload: &Value, kind: &str, lines: &mut Vec<String>) {
    if kind == "permission" && payload["toolName"] == "ExitPlanMode" {
        lines.push(format!(
            "Plan file: {}",
            safe_label(
                payload["input"]["planFilePath"]
                    .as_str()
                    .unwrap_or("not supplied")
            )
        ));
        lines.push(format!(
            "Plan:\n{}",
            safe_text(
                payload["input"]["plan"]
                    .as_str()
                    .unwrap_or("Claude supplied no plan text; decline to request a plan")
            )
        ));
    } else if kind == "permission" && payload["toolName"] != "AskUserQuestion" {
        lines.push(serde_json::to_string_pretty(&payload["input"]).unwrap_or_default());
    }
}
