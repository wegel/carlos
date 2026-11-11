//! Shows native notices without treating recoverable warnings as failed sessions.

use serde_json::Value;

use super::claude_activity::Compaction;
use super::claude_question_state::safe_label;
use super::models::{MessageKind, Role};
use super::state::AppState;

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
                app.ralph_runtime.stop_after_failure();
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
    let boundary = message["subtype"] == "compact_boundary";
    let start = message["status"] == "compacting";
    let previous_finished = app
        .claude
        .activity
        .compact
        .as_ref()
        .is_some_and(|compact| compact.result.is_some());
    let new_boundary = boundary
        && app.claude.activity.compact.as_ref().is_some_and(|compact| {
            compact
                .boundary
                .as_deref()
                .is_some_and(|id| Some(id) != message["uuid"].as_str())
        });
    if app.claude.activity.compact.is_none() || (start && previous_finished) || new_boundary {
        app.claude.activity.compact_sequence += 1;
        app.claude.activity.compact = Some(Compaction {
            key: format!(
                "compact:{}:{}",
                app.claude.generation, app.claude.activity.compact_sequence
            ),
            boundary: None,
            result: None,
        });
    }
    let compact = app
        .claude
        .activity
        .compact
        .as_mut()
        .expect("compaction was created");
    if boundary {
        compact.boundary = message["uuid"].as_str().map(str::to_string);
        compact.result = Some("success".to_string());
    }
    if let Some(result) = message["compact_result"].as_str() {
        compact.result = Some(result.to_string());
    }
    let text = match compact.result.as_deref() {
        Some("failed") => format!(
            "Claude could not compact context: {}",
            safe_label(message["compact_error"].as_str().unwrap_or("unknown error"))
        ),
        Some("success") => "Claude compacted context".to_string(),
        _ => "Claude is compacting context".to_string(),
    };
    let key = compact.key.clone();
    app.claude.activity.transient = (compact.result.is_none()).then(|| "Compacting".to_string());
    super::claude_context::invalidate(app);
    app.upsert_mapped_message(&key, Role::System, text, MessageKind::Plain, None);
}

fn notice(app: &mut AppState, message: &Value, text: &str) {
    if text.is_empty() {
        return;
    }
    let key = message["uuid"]
        .as_str()
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
