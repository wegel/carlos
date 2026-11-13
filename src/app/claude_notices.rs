//! Format native notices for this terminal and apply session redraw reports.

use serde_json::Value;

use super::state::AppState;
use carlos_core::session::claude::CompactionResult;
use carlos_core::session::models::MessageKind;
use carlos_core::session::models::Role;
use carlos_core::session::text::safe_label;

/// Show one escaped copy of a session-owned native notice.
pub(super) fn show(app: &mut AppState, key: String, text: String) {
    let update = app.session.record_claude_notice(key, &text);
    app.apply_session_update(update);
}

/// Apply native warning decisions and redraw reports.
pub(super) fn receive(app: &mut AppState, message: &Value) {
    let notices = app.session.receive_claude_notice(message);
    for (key, text) in notices {
        show(app, key, text);
    }
}

/// Show the native usage rows selected and bounded by the session.
pub(super) fn usage(app: &mut AppState, message: &Value) {
    for (key, text) in app.session.claude_usage_notice(message) {
        show(app, key, text);
    }
}

/// Update one compaction notice per attempt and invalidate stale context counts.
pub(super) fn compact(app: &mut AppState, message: &Value) {
    let notice = app.session.compact_claude_context(message);
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
