//! Style conversion boilerplate: ratatui ↔ ratatui-core colour, modifier, and style adapters,
//! plus the `CarlosMarkdownStyleSheet` implementation.

use ratatui::style::{Color, Modifier, Style};
use ratatui_core::style::{Color as CoreColor, Modifier as CoreModifier, Style as CoreStyle};

use crate::theme::*;

// --- Colour Conversion ---

pub(super) fn color_to_core(color: Color) -> CoreColor {
    match color {
        Color::Reset => CoreColor::Reset,
        Color::Black => CoreColor::Black,
        Color::Red => CoreColor::Red,
        Color::Green => CoreColor::Green,
        Color::Yellow => CoreColor::Yellow,
        Color::Blue => CoreColor::Blue,
        Color::Magenta => CoreColor::Magenta,
        Color::Cyan => CoreColor::Cyan,
        Color::Gray => CoreColor::Gray,
        Color::DarkGray => CoreColor::DarkGray,
        Color::LightRed => CoreColor::LightRed,
        Color::LightGreen => CoreColor::LightGreen,
        Color::LightYellow => CoreColor::LightYellow,
        Color::LightBlue => CoreColor::LightBlue,
        Color::LightMagenta => CoreColor::LightMagenta,
        Color::LightCyan => CoreColor::LightCyan,
        Color::White => CoreColor::White,
        Color::Rgb(r, g, b) => CoreColor::Rgb(r, g, b),
        Color::Indexed(v) => CoreColor::Indexed(v),
    }
}

pub(super) fn core_color_to_color(color: CoreColor) -> Color {
    match color {
        CoreColor::Reset => Color::Reset,
        CoreColor::Black => Color::Black,
        CoreColor::Red => Color::Red,
        CoreColor::Green => Color::Green,
        CoreColor::Yellow => Color::Yellow,
        CoreColor::Blue => Color::Blue,
        CoreColor::Magenta => Color::Magenta,
        CoreColor::Cyan => Color::Cyan,
        CoreColor::Gray => Color::Gray,
        CoreColor::DarkGray => Color::DarkGray,
        CoreColor::LightRed => Color::LightRed,
        CoreColor::LightGreen => Color::LightGreen,
        CoreColor::LightYellow => Color::LightYellow,
        CoreColor::LightBlue => Color::LightBlue,
        CoreColor::LightMagenta => Color::LightMagenta,
        CoreColor::LightCyan => Color::LightCyan,
        CoreColor::White => Color::White,
        CoreColor::Rgb(r, g, b) => Color::Rgb(r, g, b),
        CoreColor::Indexed(v) => Color::Indexed(v),
    }
}

// --- Modifier Conversion ---

pub(super) fn modifier_to_core(modifier: Modifier) -> CoreModifier {
    let mut out = CoreModifier::empty();
    if modifier.contains(Modifier::BOLD) {
        out |= CoreModifier::BOLD;
    }
    if modifier.contains(Modifier::DIM) {
        out |= CoreModifier::DIM;
    }
    if modifier.contains(Modifier::ITALIC) {
        out |= CoreModifier::ITALIC;
    }
    if modifier.contains(Modifier::UNDERLINED) {
        out |= CoreModifier::UNDERLINED;
    }
    if modifier.contains(Modifier::SLOW_BLINK) {
        out |= CoreModifier::SLOW_BLINK;
    }
    if modifier.contains(Modifier::RAPID_BLINK) {
        out |= CoreModifier::RAPID_BLINK;
    }
    if modifier.contains(Modifier::REVERSED) {
        out |= CoreModifier::REVERSED;
    }
    if modifier.contains(Modifier::HIDDEN) {
        out |= CoreModifier::HIDDEN;
    }
    if modifier.contains(Modifier::CROSSED_OUT) {
        out |= CoreModifier::CROSSED_OUT;
    }
    out
}

pub(super) fn core_modifier_to_modifier(modifier: CoreModifier) -> Modifier {
    let mut out = Modifier::empty();
    if modifier.contains(CoreModifier::BOLD) {
        out |= Modifier::BOLD;
    }
    if modifier.contains(CoreModifier::DIM) {
        out |= Modifier::DIM;
    }
    if modifier.contains(CoreModifier::ITALIC) {
        out |= Modifier::ITALIC;
    }
    if modifier.contains(CoreModifier::UNDERLINED) {
        out |= Modifier::UNDERLINED;
    }
    if modifier.contains(CoreModifier::SLOW_BLINK) {
        out |= Modifier::SLOW_BLINK;
    }
    if modifier.contains(CoreModifier::RAPID_BLINK) {
        out |= Modifier::RAPID_BLINK;
    }
    if modifier.contains(CoreModifier::REVERSED) {
        out |= Modifier::REVERSED;
    }
    if modifier.contains(CoreModifier::HIDDEN) {
        out |= Modifier::HIDDEN;
    }
    if modifier.contains(CoreModifier::CROSSED_OUT) {
        out |= Modifier::CROSSED_OUT;
    }
    out
}

// --- Style Conversion ---

pub(super) fn style_to_core(style: Style) -> CoreStyle {
    let mut out = CoreStyle::default();
    if let Some(fg) = style.fg {
        out = out.fg(color_to_core(fg));
    }
    if let Some(bg) = style.bg {
        out = out.bg(color_to_core(bg));
    }
    if let Some(ul) = style.underline_color {
        out = out.underline_color(color_to_core(ul));
    }
    out.add_modifier = modifier_to_core(style.add_modifier);
    out.sub_modifier = modifier_to_core(style.sub_modifier);
    out
}

pub(super) fn core_style_to_style(style: CoreStyle) -> Style {
    let mut out = Style::default();
    if let Some(fg) = style.fg {
        out = out.fg(core_color_to_color(fg));
    }
    if let Some(bg) = style.bg {
        out = out.bg(core_color_to_color(bg));
    }
    if let Some(ul) = style.underline_color {
        out = out.underline_color(core_color_to_color(ul));
    }
    out.add_modifier = core_modifier_to_modifier(style.add_modifier);
    out.sub_modifier = core_modifier_to_modifier(style.sub_modifier);
    out
}

// --- Markdown Stylesheet ---

#[derive(Debug, Clone, Copy)]
pub(super) struct CarlosMarkdownStyleSheet;

impl tui_markdown::StyleSheet for CarlosMarkdownStyleSheet {
    fn heading(&self, level: u8) -> CoreStyle {
        match level {
            1 => style_to_core(
                Style::default()
                    .fg(COLOR_TEXT)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            ),
            2 => style_to_core(Style::default().fg(COLOR_TEXT).add_modifier(Modifier::BOLD)),
            _ => style_to_core(Style::default().fg(COLOR_TEXT)),
        }
    }

    fn code(&self) -> CoreStyle {
        style_to_core(Style::default().fg(COLOR_TEXT))
    }

    fn link(&self) -> CoreStyle {
        style_to_core(
            Style::default()
                .fg(COLOR_GUTTER_USER)
                .add_modifier(Modifier::UNDERLINED),
        )
    }

    fn blockquote(&self) -> CoreStyle {
        style_to_core(Style::default().fg(COLOR_DIM))
    }

    fn heading_meta(&self) -> CoreStyle {
        style_to_core(Style::default().fg(COLOR_DIM).add_modifier(Modifier::DIM))
    }

    fn metadata_block(&self) -> CoreStyle {
        style_to_core(Style::default().fg(COLOR_DIM))
    }
}
