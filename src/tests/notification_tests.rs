use serde_json::json;

use super::{
    append_history_from_thread, build_rendered_lines, handle_notification_line,
    handle_server_message_line, load_history_from_start_or_resume, AppState, ContextUsage,
    MessageKind, Role, ServerRequestAction,
};

include!("notification_tests/part_1.rs");

include!("notification_tests/part_2.rs");

include!("notification_tests/part_3.rs");
