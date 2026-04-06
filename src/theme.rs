//! Catppuccin Mocha colour palette and role-based styling constants.

use ratatui::style::Color;

use crate::app::Role;

// Catppuccin Mocha defaults
pub(crate) const COLOR_STEP1: Color = Color::Rgb(17, 17, 27); // crust
pub(crate) const COLOR_STEP2: Color = Color::Rgb(24, 24, 37); // mantle
pub(crate) const COLOR_STEP3: Color = Color::Rgb(30, 30, 46); // base
pub(crate) const COLOR_STEP6: Color = Color::Rgb(49, 50, 68); // surface0
pub(crate) const COLOR_STEP7: Color = Color::Rgb(69, 71, 90); // surface1
pub(crate) const COLOR_STEP8: Color = Color::Rgb(108, 112, 134); // overlay0
pub(crate) const COLOR_PRIMARY: Color = Color::Rgb(203, 166, 247); // mauve
pub(crate) const COLOR_TEXT: Color = Color::Rgb(205, 214, 244); // text
pub(crate) const COLOR_DIM: Color = Color::Rgb(166, 173, 200); // subtext0
pub(crate) const COLOR_OVERLAY: Color = Color::Rgb(17, 17, 27);

pub(crate) const COLOR_ROW_USER: Color = Color::Rgb(34, 36, 54);
pub(crate) const COLOR_ROW_AGENT_OUTPUT: Color = COLOR_ROW_SYSTEM;
pub(crate) const COLOR_ROW_AGENT_COMMENTARY: Color = Color::Rgb(34, 36, 48);
pub(crate) const COLOR_ROW_AGENT_THINKING: Color = Color::Rgb(40, 42, 60);
pub(crate) const COLOR_ROW_TOOL_CALL: Color = Color::Rgb(44, 40, 58);
pub(crate) const COLOR_ROW_TOOL_OUTPUT: Color = Color::Rgb(38, 43, 53);
pub(crate) const COLOR_ROW_SYSTEM: Color = COLOR_STEP2;

pub(crate) const COLOR_GUTTER_USER: Color = Color::Rgb(137, 180, 250); // blue
pub(crate) const COLOR_GUTTER_AGENT_OUTPUT: Color = Color::Rgb(166, 227, 161); // green
pub(crate) const COLOR_GUTTER_AGENT_COMMENTARY: Color = Color::Rgb(137, 220, 235); // sky
pub(crate) const COLOR_GUTTER_AGENT_THINKING: Color = Color::Rgb(245, 194, 231); // pink
pub(crate) const COLOR_GUTTER_TOOL_CALL: Color = Color::Rgb(250, 179, 135); // peach
pub(crate) const COLOR_GUTTER_TOOL_OUTPUT: Color = Color::Rgb(250, 179, 135); // peach
pub(crate) const COLOR_GUTTER_SYSTEM: Color = Color::Rgb(137, 220, 235); // sky
pub(crate) const COLOR_DIFF_ADD: Color = Color::Rgb(166, 227, 161); // green
pub(crate) const COLOR_DIFF_REMOVE: Color = Color::Rgb(243, 139, 168); // red
pub(crate) const COLOR_DIFF_HUNK: Color = Color::Rgb(250, 179, 135); // peach
pub(crate) const COLOR_DIFF_HEADER: Color = Color::Rgb(137, 220, 235); // sky
pub(crate) const COLOR_KITT_HEAD: Color = Color::Rgb(137, 220, 235); // sky
pub(crate) const COLOR_KITT_TRAIL_1: Color = Color::Rgb(116, 199, 236); // sapphire
pub(crate) const COLOR_KITT_TRAIL_2: Color = Color::Rgb(137, 180, 250); // blue
pub(crate) const COLOR_KITT_TRAIL_3: Color = Color::Rgb(108, 112, 134); // overlay0
pub(crate) const COLOR_KITT_BASE: Color = COLOR_STEP7;
pub(crate) const COLOR_KITT_RALPH_HEAD: Color = Color::Rgb(245, 194, 231); // pink
pub(crate) const COLOR_KITT_RALPH_TRAIL_1: Color = Color::Rgb(242, 205, 205); // flamingo
pub(crate) const COLOR_KITT_RALPH_TRAIL_2: Color = Color::Rgb(203, 166, 247); // mauve
pub(crate) const COLOR_KITT_RALPH_TRAIL_3: Color = Color::Rgb(108, 112, 134); // overlay0
pub(crate) const KITT_STEP_MS: u128 = 45;
pub(crate) const TOUCH_SCROLL_DRAG_MIN_ROWS: usize = 3;

pub(crate) fn role_fg(role: Role) -> Color {
    match role {
        Role::User => COLOR_TEXT,
        Role::Assistant => COLOR_TEXT,
        Role::Commentary => COLOR_DIM,
        Role::Reasoning => COLOR_TEXT,
        Role::ToolCall => COLOR_TEXT,
        Role::ToolOutput => COLOR_TEXT,
        Role::System => COLOR_DIM,
    }
}

pub(crate) fn role_gutter_fg(role: Role) -> Color {
    match role {
        Role::User => COLOR_GUTTER_USER,
        Role::Assistant => COLOR_GUTTER_AGENT_OUTPUT,
        Role::Commentary => COLOR_GUTTER_AGENT_COMMENTARY,
        Role::Reasoning => COLOR_GUTTER_AGENT_THINKING,
        Role::ToolCall => COLOR_GUTTER_TOOL_CALL,
        Role::ToolOutput => COLOR_GUTTER_TOOL_OUTPUT,
        Role::System => COLOR_GUTTER_SYSTEM,
    }
}

pub(crate) fn role_row_bg(role: Role) -> Color {
    match role {
        Role::User => COLOR_ROW_USER,
        Role::Assistant => COLOR_ROW_AGENT_OUTPUT,
        Role::Commentary => COLOR_ROW_AGENT_COMMENTARY,
        Role::Reasoning => COLOR_ROW_AGENT_THINKING,
        Role::ToolCall => COLOR_ROW_TOOL_CALL,
        Role::ToolOutput => COLOR_ROW_TOOL_OUTPUT,
        Role::System => COLOR_ROW_SYSTEM,
    }
}

pub(crate) fn role_gutter_symbol(role: Role) -> &'static str {
    match role {
        Role::Assistant => " ",
        Role::Commentary => "·",
        _ => "┃",
    }
}

pub(crate) fn kitt_color_for_distance(distance: usize, ralph_mode: bool) -> Color {
    if ralph_mode {
        return match distance {
            0 => COLOR_KITT_RALPH_HEAD,
            1 => COLOR_KITT_RALPH_TRAIL_1,
            2 => COLOR_KITT_RALPH_TRAIL_2,
            3 => COLOR_KITT_RALPH_TRAIL_3,
            _ => COLOR_KITT_BASE,
        };
    }

    match distance {
        0 => COLOR_KITT_HEAD,
        1 => COLOR_KITT_TRAIL_1,
        2 => COLOR_KITT_TRAIL_2,
        3 => COLOR_KITT_TRAIL_3,
        _ => COLOR_KITT_BASE,
    }
}
