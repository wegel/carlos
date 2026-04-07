//! Style conversion boilerplate: ratatui ↔ ratatui-core colour, modifier, and style adapters,
//! plus the `CarlosMarkdownStyleSheet` implementation.

use ratatui::style::{Color, Modifier, Style};
use ratatui_core::style::{Color as CoreColor, Modifier as CoreModifier, Style as CoreStyle};

use crate::theme::{COLOR_DIM, COLOR_GUTTER_USER, COLOR_TEXT};

macro_rules! convert_enum {
    (
        $to_core_fn:ident,
        $from_core_fn:ident,
        $from_ty:ident,
        $to_ty:ident,
        [$($unit_variant:ident),* $(,)?],
        [$($data_variant:ident($($field:ident),+)),* $(,)?]
    ) => {
        pub(super) fn $to_core_fn(value: $from_ty) -> $to_ty {
            match value {
                $($from_ty::$unit_variant => $to_ty::$unit_variant,)*
                $($from_ty::$data_variant($($field),+) => $to_ty::$data_variant($($field),+),)*
            }
        }

        pub(super) fn $from_core_fn(value: $to_ty) -> $from_ty {
            match value {
                $($to_ty::$unit_variant => $from_ty::$unit_variant,)*
                $($to_ty::$data_variant($($field),+) => $from_ty::$data_variant($($field),+),)*
            }
        }
    };
}

macro_rules! convert_modifiers {
    ($to_core_fn:ident, $from_core_fn:ident, $from_ty:ident, $to_ty:ident, [$($flag:ident),* $(,)?]) => {
        pub(super) fn $to_core_fn(value: $from_ty) -> $to_ty {
            let mut out = $to_ty::empty();
            $(if value.contains($from_ty::$flag) { out |= $to_ty::$flag; })*
            out
        }

        pub(super) fn $from_core_fn(value: $to_ty) -> $from_ty {
            let mut out = $from_ty::empty();
            $(if value.contains($to_ty::$flag) { out |= $from_ty::$flag; })*
            out
        }
    };
}

macro_rules! convert_style {
    (
        $to_core_fn:ident,
        $from_core_fn:ident,
        $from_ty:ident,
        $to_ty:ident,
        $color_to_core_fn:ident,
        $color_from_core_fn:ident,
        $modifier_to_core_fn:ident,
        $modifier_from_core_fn:ident,
        [$($field:ident => $setter:ident),* $(,)?]
    ) => {
        pub(super) fn $to_core_fn(style: $from_ty) -> $to_ty {
            let mut out = $to_ty::default();
            $(if let Some(value) = style.$field { out = out.$setter($color_to_core_fn(value)); })*
            out.add_modifier = $modifier_to_core_fn(style.add_modifier);
            out.sub_modifier = $modifier_to_core_fn(style.sub_modifier);
            out
        }

        pub(super) fn $from_core_fn(style: $to_ty) -> $from_ty {
            let mut out = $from_ty::default();
            $(if let Some(value) = style.$field { out = out.$setter($color_from_core_fn(value)); })*
            out.add_modifier = $modifier_from_core_fn(style.add_modifier);
            out.sub_modifier = $modifier_from_core_fn(style.sub_modifier);
            out
        }
    };
}

// --- Colour Conversion ---

convert_enum!(
    color_to_core,
    core_color_to_color,
    Color,
    CoreColor,
    [
        Reset, Black, Red, Green, Yellow, Blue, Magenta, Cyan, Gray, DarkGray, LightRed,
        LightGreen, LightYellow, LightBlue, LightMagenta, LightCyan, White
    ],
    [Rgb(r, g, b), Indexed(v)]
);

// --- Modifier Conversion ---

convert_modifiers!(
    modifier_to_core,
    core_modifier_to_modifier,
    Modifier,
    CoreModifier,
    [BOLD, DIM, ITALIC, UNDERLINED, SLOW_BLINK, RAPID_BLINK, REVERSED, HIDDEN, CROSSED_OUT]
);

// --- Style Conversion ---

convert_style!(
    style_to_core,
    core_style_to_style,
    Style,
    CoreStyle,
    color_to_core,
    core_color_to_color,
    modifier_to_core,
    core_modifier_to_modifier,
    [fg => fg, bg => bg, underline_color => underline_color]
);

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
