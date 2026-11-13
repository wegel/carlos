//! Notification subsystem test tree: context, approvals, history, tools, diffs.

use carlos_core::session::context_usage::ContextUsage;
use carlos_core::session::models::MessageKind;
use carlos_core::session::models::Role;
use serde_json::json;

use super::{
    append_history_from_thread, build_rendered_lines, handle_notification_line,
    handle_server_message_line, load_history_from_start_or_resume, AppState, ServerRequestAction,
};

include!("notification_tests/context_and_approvals.rs");

include!("notification_tests/history_and_tools.rs");

include!("notification_tests/diff_notifications.rs");
