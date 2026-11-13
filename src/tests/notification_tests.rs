//! Notification subsystem test tree: context, approvals, history, tools, diffs.

use serde_json::json;

use super::{
    append_history_from_thread, build_rendered_lines, handle_notification_line,
    handle_server_message_line, load_history_from_start_or_resume, AppState, ServerRequestAction,
};
use crate::session::context_usage::ContextUsage;
use crate::session::models::MessageKind;
use crate::session::models::Role;

include!("notification_tests/context_and_approvals.rs");

include!("notification_tests/history_and_tools.rs");

include!("notification_tests/diff_notifications.rs");
