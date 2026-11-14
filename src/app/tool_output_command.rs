//! Handle tool-output commands locally without sending a chat turn.

use carlos_core::session::models::Role;

use super::state::AppState;
use super::tool_output_preferences::save_mode;
use super::tool_output_state::ToolOutputMode;

/// Consume `/tools [auto|compact|full]`; a bare command cycles the three modes.
pub(super) fn handle_tools_command(app: &mut AppState, text: &str) -> bool {
    let Some(mode) = parse_tools_command(app.view.tool_output.mode, text) else {
        return false;
    };
    match mode {
        Ok(mode) => {
            app.view.tool_output.mode = mode;
            app.clear_input();
            if let Err(error) = save_mode(mode) {
                app.append_message(
                    Role::System,
                    format!("Tool output changed; could not save choice: {error}"),
                );
            }
        }
        Err(message) => {
            app.append_message(Role::System, message);
        }
    }
    true
}

/// Parse a local display command independently of any backend or session owner.
pub(super) fn parse_tools_command(
    current: ToolOutputMode,
    text: &str,
) -> Option<Result<ToolOutputMode, &'static str>> {
    let rest = text.trim().strip_prefix("/tools")?;
    if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let mode = if rest.trim().is_empty() {
        Some(match current {
            ToolOutputMode::Auto => ToolOutputMode::Compact,
            ToolOutputMode::Compact => ToolOutputMode::Full,
            ToolOutputMode::Full => ToolOutputMode::Auto,
        })
    } else {
        ToolOutputMode::parse(rest.trim())
    };
    Some(mode.ok_or("Use /tools auto, /tools compact, or /tools full"))
}
