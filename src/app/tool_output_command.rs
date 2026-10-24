//! Handle tool-output commands locally without sending a chat turn.

use super::models::Role;
use super::state::AppState;
use super::tool_output_preferences::save_mode;
use super::tool_output_state::ToolOutputMode;

/// Consume `/tools [auto|compact|full]`; a bare command cycles the three modes.
pub(super) fn handle_tools_command(app: &mut AppState, text: &str) -> bool {
    let Some(rest) = text.trim().strip_prefix("/tools") else {
        return false;
    };
    if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
        return false;
    }
    let mode = if rest.trim().is_empty() {
        Some(match app.tool_output.mode {
            ToolOutputMode::Auto => ToolOutputMode::Compact,
            ToolOutputMode::Compact => ToolOutputMode::Full,
            ToolOutputMode::Full => ToolOutputMode::Auto,
        })
    } else {
        ToolOutputMode::parse(rest.trim())
    };
    match mode {
        Some(mode) => {
            app.tool_output.mode = mode;
            app.clear_input();
            if let Err(error) = save_mode(mode) {
                app.append_message(
                    Role::System,
                    format!("Tool output changed; could not save choice: {error}"),
                );
            }
        }
        None => {
            app.append_message(
                Role::System,
                "Use /tools auto, /tools compact, or /tools full",
            );
        }
    }
    true
}
