use serde_json::json;

use super::{
    format_tool_item, handle_notification_line, parse_ssh_remote_command, strip_terminal_controls,
    strip_terminal_controls_preserving_sgr, AppState, MessageKind, Role,
};

include!("tool_tests/part_1.rs");

include!("tool_tests/part_2.rs");
