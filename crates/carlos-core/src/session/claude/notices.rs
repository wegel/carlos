//! Choose native warning defaults and retain a bounded FIFO of emitted notice keys.

use serde_json::Value;

use crate::session::models::Role;
use crate::session::state::SessionState;
use crate::session::text::{safe_label, safe_text};

impl SessionState {
    /// Append one escaped notice, retaining the original FIFO and dedup order.
    pub fn record_claude_notice(&mut self, key: String, text: &str) {
        if self.claude.notices.contains(&key) {
            return;
        }
        self.claude.notices.push_back(key);
        if self.claude.notices.len() > 128 {
            self.claude.notices.pop_front();
        }
        self.append_message(Role::System, safe_text(text));
    }

    /// Handle known native warnings and keep unknown additive system messages harmless.
    pub fn receive_claude_notice(&mut self, message: &Value) -> Vec<(String, String)> {
        let mut notices = Vec::new();
        match message["subtype"].as_str() {
            Some("init") => {
                plugins(&mut notices, message);
            }
            Some("informational") => {
                if message["prevent_continuation"] == true {
                    self.ralph_stop_after_failure();
                }
                notice(
                    &mut notices,
                    message,
                    message["content"].as_str().unwrap_or(""),
                );
            }
            Some("session_writes_lost") => {
                notice(&mut notices,message,&format!("Claude could not save session history ({}). Received replies remain visible; save anything you need. Lost writes: {}",safe_label(message["error_code"].as_str().unwrap_or("unknown error")),message["lost"]));
            }
            Some("plugin_install") => {
                if message["status"] == "failed" {
                    notice(
                        &mut notices,
                        message,
                        &format!(
                            "Plugin {} failed: {}",
                            safe_label(message["name"].as_str().unwrap_or("unknown")),
                            safe_label(message["error"].as_str().unwrap_or("unknown error"))
                        ),
                    );
                }
            }
            Some("hook_response")
                if (message["outcome"] != "success"
                    || message["exit_code"].as_i64().is_some_and(|code| code != 0)) =>
            {
                notice(
                    &mut notices,
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
            _ => {}
        }
        notices
    }

    /// Show the server's stable usage-report rows with their supplied scope and severity.
    pub fn claude_usage_notice(&self, message: &Value) -> Vec<(String, String)> {
        let Some(rows) = message["usage_report"]["rate_limits"]["limits"].as_array() else {
            return Vec::new();
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
        if lines.is_empty() {
            Vec::new()
        } else {
            vec![(format!("usage:{}", message["uuid"]), lines.join("\n"))]
        }
    }
}

fn notice(notices: &mut Vec<(String, String)>, message: &Value, text: &str) {
    if text.is_empty() {
        return;
    }
    let key = message["uuid"]
        .as_str()
        .or_else(|| message["noticeKey"].as_str())
        .map(str::to_string)
        .unwrap_or_else(|| message.to_string());
    notices.push((key, text.to_string()));
}

fn plugins(notices: &mut Vec<(String, String)>, message: &Value) {
    if let Some(errors) = message["plugin_errors"].as_array() {
        for error in errors {
            notices.push((
                format!("plugin:{error}"),
                format!(
                    "Plugin {}: {} ({})",
                    safe_label(error["plugin"].as_str().unwrap_or("unknown")),
                    safe_label(error["message"].as_str().unwrap_or("could not load")),
                    safe_label(error["path"].as_str().unwrap_or("unknown path"))
                ),
            ));
        }
    }
}
