//! Share status-label placement, cell colors and activity sweeps between terminal adapters.

use carlos_core::session::context_usage::ContextUsage;
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier, Style};

use super::context_usage::{context_usage_label, context_usage_placeholder_label};
use super::notifications::kitt_head_index;
use super::render::draw_str;
use super::text::{split_at_cells, visual_width};
use super::weekly_usage::WeeklyIndicator;
use crate::theme::{
    context_color, kitt_color_for_distance, COLOR_GUTTER_USER, COLOR_STEP8, COLOR_TOOL_TEXT,
};

pub(super) struct Label {
    pub(super) parts: Vec<(String, Color)>,
    pub(super) bold: bool,
}

impl Label {
    pub(super) fn plain(text: String, color: Color) -> Self {
        Self {
            parts: vec![(text, color)],
            bold: false,
        }
    }

    pub(super) fn width(&self) -> usize {
        self.parts.iter().map(|(text, _)| visual_width(text)).sum()
    }
}

pub(super) struct PlacedLabel {
    pub(super) x: usize,
    pub(super) label: Label,
}

pub(super) fn place_weekly(
    labels: &mut Vec<PlacedLabel>,
    edge: &mut usize,
    weekly: WeeklyIndicator,
) {
    place(
        labels,
        edge,
        Label {
            parts: vec![
                ("W".to_string(), COLOR_STEP8),
                (weekly.percent, weekly.color),
                (format!("/{}", weekly.countdown), COLOR_STEP8),
            ],
            bold: false,
        },
    );
}

pub(super) fn place_context(
    labels: &mut Vec<PlacedLabel>,
    edge: &mut usize,
    usage: Option<ContextUsage>,
    stale: bool,
    reserve: usize,
) {
    let context = usage
        .map(context_usage_label)
        .unwrap_or_else(|| context_usage_placeholder_label().to_string());
    let stale = stale && usage.is_some();
    let context = if stale {
        format!("~{context}")
    } else {
        context
    };
    let (tokens, percent) = context.split_once(" (").unwrap_or((&context, ""));
    let percent = percent.trim_end_matches(')');
    let color = usage.map_or(COLOR_GUTTER_USER, |u| context_color(u.used, u.max));
    let full = Label {
        parts: vec![
            (format!("{tokens} "), COLOR_TOOL_TEXT),
            (percent.to_owned(), color),
        ],
        bold: false,
    };
    if full.width() + 2 + reserve > *edge || !place(labels, edge, full) {
        place(
            labels,
            edge,
            Label::plain(
                if stale {
                    format!("~{percent}")
                } else {
                    percent.to_owned()
                },
                color,
            ),
        );
    }
}

pub(super) fn fit_model(text: &str, width: usize) -> String {
    if visual_width(text) <= width {
        text.to_string()
    } else if width > 0 {
        format!("{}…", &text[..split_at_cells(text, width - 1)])
    } else {
        String::new()
    }
}

pub(super) fn place(labels: &mut Vec<PlacedLabel>, edge: &mut usize, label: Label) -> bool {
    let width = label.width();
    if width == 0 || width + 2 > *edge {
        return false;
    }
    let x = *edge - width;
    *edge = x - 1;
    labels.push(PlacedLabel { x, label });
    true
}

pub(super) fn draw_labels(buf: &mut Buffer, y: usize, labels: &[PlacedLabel]) {
    for placed in labels {
        let mut x = placed.x;
        for (text, color) in &placed.label.parts {
            let mut style = Style::default().fg(*color);
            if placed.label.bold {
                style = style.add_modifier(Modifier::BOLD);
            }
            draw_str(buf, x, y, text, style, visual_width(text));
            x += visual_width(text);
        }
    }
}

pub(super) fn sweep_status_line(
    buf: &mut Buffer,
    y: usize,
    width: usize,
    ralph_mode: bool,
    tick: u128,
) {
    let head = kitt_head_index(width, tick);
    for x in 0..width {
        let distance = head.abs_diff(x);
        if distance <= 3 {
            buf[(x as u16, y as u16)].fg = kitt_color_for_distance(distance, ralph_mode);
        }
        if distance == 0 {
            buf[(x as u16, y as u16)].modifier.insert(Modifier::BOLD);
        }
    }
}

#[cfg(test)]
#[path = "../tests/status_sweep_tests.rs"]
mod status_sweep_tests;
