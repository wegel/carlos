//! The 3c transcript theme, built from Catppuccin Mocha colors.

use ratatui::style::{Color, Modifier, Style};

use crate::app::Role;

// Catppuccin Mocha defaults
pub(crate) const COLOR_STEP1: Color = Color::Rgb(17, 17, 27); // crust
pub(crate) const COLOR_STEP2: Color = Color::Rgb(24, 24, 37); // mantle
pub(crate) const COLOR_STEP6: Color = Color::Rgb(49, 50, 68); // surface0
pub(crate) const COLOR_STEP7: Color = Color::Rgb(69, 71, 90); // surface1
pub(crate) const COLOR_STEP8: Color = Color::Rgb(108, 112, 134); // overlay0
pub(crate) const COLOR_PRIMARY: Color = Color::Rgb(203, 166, 247); // mauve
pub(crate) const COLOR_TEXT: Color = Color::Rgb(205, 214, 244); // text
pub(crate) const COLOR_DIM: Color = Color::Rgb(166, 173, 200); // subtext0
pub(crate) const COLOR_OVERLAY: Color = Color::Rgb(17, 17, 27);

pub(crate) const COLOR_ROW_USER: Color = Color::Rgb(32, 34, 53);
pub(crate) const COLOR_REASONING: Color = Color::Rgb(172, 159, 209);
pub(crate) const COLOR_COMMENTARY: Color = Color::Rgb(219, 207, 239);
pub(crate) const COLOR_TOOL_TEXT: Color = Color::Rgb(147, 153, 178);
pub(crate) const COLOR_TOOL_READ: Color = Color::Rgb(116, 199, 236);
pub(crate) const COLOR_TOOL_SEARCH: Color = Color::Rgb(148, 226, 213);
pub(crate) const COLOR_TOOL_EDIT: Color = Color::Rgb(249, 226, 175);

pub(crate) const COLOR_GUTTER_USER: Color = Color::Rgb(137, 180, 250); // blue
pub(crate) const COLOR_GUTTER_AGENT_OUTPUT: Color = Color::Rgb(166, 227, 161); // green
pub(crate) const COLOR_GUTTER_AGENT_COMMENTARY: Color = Color::Rgb(245, 194, 231); // pink
pub(crate) const COLOR_GUTTER_AGENT_THINKING: Color = COLOR_PRIMARY;
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
pub(crate) const ANIMATION_STEP_MS: u128 = 45;
pub(crate) const TOUCH_SCROLL_DRAG_MIN_ROWS: usize = 3;

pub(crate) fn role_fg(role: Role) -> Color {
    match role {
        Role::User => COLOR_TEXT,
        Role::Assistant => COLOR_TEXT,
        Role::Commentary => COLOR_COMMENTARY,
        Role::Reasoning => COLOR_REASONING,
        Role::ToolCall | Role::ToolOutput | Role::System => COLOR_TOOL_TEXT,
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
        _ => COLOR_STEP2,
    }
}

pub(crate) fn role_gutter_symbol(role: Role) -> &'static str {
    match role {
        Role::User => "❯",
        Role::Assistant => "✓",
        Role::Commentary => "•",
        Role::Reasoning => "◇",
        Role::ToolCall | Role::ToolOutput => "$",
        Role::System => "·",
    }
}

pub(crate) fn role_text_style(role: Role) -> Style {
    let style = Style::default().fg(role_fg(role));
    match role {
        Role::User => style.add_modifier(Modifier::BOLD),
        Role::Reasoning => style.add_modifier(Modifier::ITALIC),
        _ => style,
    }
}

/// Use the unrounded fraction so the warning starts strictly above each threshold.
pub(crate) fn context_color(used: u64, max: u64) -> Color {
    let used = u128::from(used) * 100;
    let max = u128::from(max);
    if max > 0 && used > max * 90 {
        COLOR_DIFF_REMOVE
    } else if max > 0 && used > max * 70 {
        COLOR_TOOL_EDIT
    } else {
        COLOR_GUTTER_USER
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
