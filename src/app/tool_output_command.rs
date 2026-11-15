//! Handle tool-output commands locally without sending a chat turn.

use super::tool_output_state::ToolOutputMode;

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
