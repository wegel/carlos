//! Seed a bounded upstream ANSI parse with the exact style saved at a source checkpoint.

use ratatui::style::{Color, Modifier, Style};

use crate::app::transcript_styles::ansi_line_segments;
use crate::app::view_models::StyledSegment;

const SENTINEL: &str = "\u{e000}";

/// Keep parsed physical lines and the exact style at their trailing byte boundary.
pub(super) struct Batch {
    pub lines: Vec<Vec<StyledSegment>>,
    pub after: Style,
}

/// A sentinel on its own next line exposes styles from control-only trailing text too.
pub(super) fn parse(raw: &str, before: Style) -> Option<Batch> {
    let mut input = seed(before);
    input.push_str(raw);
    if !raw.ends_with(['\r', '\n']) {
        input.push('\n');
    }
    input.push_str(SENTINEL);
    let mut lines = ansi_line_segments(&input)?;
    let sentinel = lines.pop()?;
    if sentinel.len() != 1 || sentinel[0].text != SENTINEL {
        return None;
    }
    Some(Batch {
        lines,
        after: sentinel[0].style,
    })
}

fn seed(style: Style) -> String {
    let mut codes = Vec::new();
    // Upstream can set underline color only through a complete reset.
    if style.underline_color == Some(Color::Reset) {
        codes.push("0".into());
    }
    if let Some(color) = style.fg {
        codes.push(color_code(color, false));
    }
    if let Some(color) = style.bg {
        codes.push(color_code(color, true));
    }
    for (flags, code) in [
        (Modifier::BOLD | Modifier::DIM, 22),
        (Modifier::ITALIC, 23),
        (Modifier::UNDERLINED, 24),
        (Modifier::SLOW_BLINK | Modifier::RAPID_BLINK, 25),
        (Modifier::REVERSED, 27),
        (Modifier::HIDDEN, 28),
        (Modifier::CROSSED_OUT, 29),
    ] {
        if style.sub_modifier.intersects(flags) {
            codes.push(code.to_string());
        }
    }
    for (flag, code) in [
        (Modifier::BOLD, 1),
        (Modifier::DIM, 2),
        (Modifier::ITALIC, 3),
        (Modifier::UNDERLINED, 4),
        (Modifier::SLOW_BLINK, 5),
        (Modifier::RAPID_BLINK, 6),
        (Modifier::REVERSED, 7),
        (Modifier::HIDDEN, 8),
        (Modifier::CROSSED_OUT, 9),
    ] {
        if style.add_modifier.contains(flag) {
            codes.push(code.to_string());
        }
    }
    if codes.is_empty() {
        String::new()
    } else {
        format!("\x1b[{}m", codes.join(";"))
    }
}

fn color_code(color: Color, background: bool) -> String {
    let base = if background { 48 } else { 38 };
    match color {
        Color::Rgb(r, g, b) => format!("{base};2;{r};{g};{b}"),
        Color::Indexed(index) => format!("{base};5;{index}"),
        color => {
            let foreground = match color {
                Color::Reset => 39,
                Color::Black => 30,
                Color::Red => 31,
                Color::Green => 32,
                Color::Yellow => 33,
                Color::Blue => 34,
                Color::Magenta => 35,
                Color::Cyan => 36,
                Color::Gray => 37,
                Color::DarkGray => 90,
                Color::LightRed => 91,
                Color::LightGreen => 92,
                Color::LightYellow => 93,
                Color::LightBlue => 94,
                Color::LightMagenta => 95,
                Color::LightCyan => 96,
                Color::White => 97,
                Color::Rgb(..) | Color::Indexed(_) => unreachable!("handled extended colors"),
            };
            (foreground + if background { 10 } else { 0 }).to_string()
        }
    }
}
