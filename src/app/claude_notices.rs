//! Shows native notices without treating recoverable warnings as failed sessions.

use serde_json::Value;

use super::claude_question_state::safe_label;
use super::models::{MessageKind, Role};
use super::state::AppState;
use crate::session::claude::CompactionResult;

/// Show one copy of a native notice while bounding the duplicate-key cache.
pub(super) fn show(app: &mut AppState, key: String, text: String) {
    if app.claude.notices.contains(&key) {
        return;
    }
    app.claude.notices.push_back(key);
    if app.claude.notices.len() > 128 {
        app.claude.notices.pop_front();
    }
    app.append_message(Role::System, super::claude_question_state::safe_text(&text));
}

/// Handle known native warnings and keep unknown additive system messages harmless.
pub(super) fn receive(app: &mut AppState, message: &Value) -> bool {
    match message["subtype"].as_str() {
        Some("init") => {
            plugins(app, message);
            false
        }
        Some("informational") => {
            if message["prevent_continuation"] == true {
                app.session.ralph_stop_after_failure();
            }
            notice(app, message, message["content"].as_str().unwrap_or(""));
            true
        }
        Some("session_writes_lost") => {
            notice(app,message,&format!("Claude could not save session history ({}). Received replies remain visible; save anything you need. Lost writes: {}",safe_label(message["error_code"].as_str().unwrap_or("unknown error")),message["lost"]));
            true
        }
        Some("plugin_install") => {
            if message["status"] == "failed" {
                notice(
                    app,
                    message,
                    &format!(
                        "Plugin {} failed: {}",
                        safe_label(message["name"].as_str().unwrap_or("unknown")),
                        safe_label(message["error"].as_str().unwrap_or("unknown error"))
                    ),
                );
            }
            true
        }
        Some("hook_response") => {
            if message["outcome"] != "success"
                || message["exit_code"].as_i64().is_some_and(|code| code != 0)
            {
                notice(
                    app,
                    message,
                    &format!(
                        "Hook {} ({}): {} {}",
                        safe_label(message["hook_name"].as_str().unwrap_or("unknown")),
                        safe_label(message["outcome"].as_str().unwrap_or("unknown")),
                        message["output"].as_str().unwrap_or(""),
                        message["stderr"].as_str().unwrap_or("")
                    ),
                );
            }
            true
        }
        _ => false,
    }
}

/// Update one compaction notice per attempt and invalidate stale context counts.
pub(super) fn compact(app: &mut AppState, message: &Value) {
    let notice = app
        .session
        .compact_claude_context(message, app.claude.generation);
    let text = match notice.result {
        CompactionResult::Failed => format!(
            "Claude could not compact context: {}",
            safe_label(message["compact_error"].as_str().unwrap_or("unknown error"))
        ),
        CompactionResult::Success => "Claude compacted context".to_string(),
        CompactionResult::Pending => "Claude is compacting context".to_string(),
    };
    app.upsert_mapped_message(&notice.key, Role::System, text, MessageKind::Plain, None);
}

fn notice(app: &mut AppState, message: &Value, text: &str) {
    if text.is_empty() {
        return;
    }
    let key = message["uuid"]
        .as_str()
        .or_else(|| message["noticeKey"].as_str())
        .map(str::to_string)
        .unwrap_or_else(|| message.to_string());
    show(app, key, text.to_string());
}

/// Show the server's stable usage-report rows with their supplied scope and severity.
pub(super) fn usage(app: &mut AppState, message: &Value) {
    let Some(rows) = message["usage_report"]["rate_limits"]["limits"].as_array() else {
        return;
    };
    let mut lines = Vec::new();
    for row in rows.iter().take(128) {
        let Some(percent) = row["percent"]
            .as_f64()
            .filter(|percent| percent.is_finite() && (0.0..=100.0).contains(percent))
        else {
            continue;
        };
        let scope = row["scope"]["model"]["display_name"]
            .as_str()
            .or_else(|| row["scope"]["surface"]["display_name"].as_str())
            .unwrap_or("");
        lines.push(format!(
            "{} {}: {percent:.1}% used; resets {} ({})",
            safe_label(row["kind"].as_str().unwrap_or("quota")),
            safe_label(scope),
            safe_label(row["resets_at"].as_str().unwrap_or("unknown")),
            safe_label(row["severity"].as_str().unwrap_or("unknown"))
        ));
    }
    if !lines.is_empty() {
        show(app, format!("usage:{}", message["uuid"]), lines.join("\n"));
    }
}

fn plugins(app: &mut AppState, message: &Value) {
    if let Some(errors) = message["plugin_errors"].as_array() {
        for error in errors {
            show(
                app,
                format!("plugin:{error}"),
                format!(
                    "Plugin {}: {} ({})",
                    safe_label(error["plugin"].as_str().unwrap_or("unknown")),
                    safe_label(error["message"].as_str().unwrap_or("could not load")),
                    safe_label(error["path"].as_str().unwrap_or("unknown path"))
                ),
            );
        }
    }
}
