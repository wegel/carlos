//! Tool-rendering test tree: command summaries and shell rendering.

use serde_json::json;

use super::{
    format_tool_item, handle_notification_line, parse_ssh_remote_command, strip_terminal_controls,
    strip_terminal_controls_preserving_sgr, AppState,
};
use carlos_core::session::models::MessageKind;
use carlos_core::session::models::Role;

include!("tool_tests/command_summaries.rs");

include!("tool_tests/shell_rendering.rs");
