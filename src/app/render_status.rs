//! Fit context and weekly quota labels beside the animated status separator.

use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier, Style};

use super::context_usage::{context_usage_label, context_usage_placeholder_label};
use super::models::TerminalSize;
use super::notifications::{animation_tick, kitt_head_index};
use super::render::draw_str;
use super::state::AppState;
use super::text::{split_at_cells, visual_width};
use crate::theme::{
    context_color, kitt_color_for_distance, COLOR_DIFF_REMOVE, COLOR_GUTTER_AGENT_COMMENTARY,
    COLOR_GUTTER_AGENT_THINKING, COLOR_GUTTER_USER, COLOR_STATUS_SEPARATOR, COLOR_STEP8,
    COLOR_TOOL_TEXT,
};

#[cfg(test)]
#[path = "../tests/status_sweep_tests.rs"]
mod status_sweep_tests;

struct Label {
    parts: Vec<(String, Color)>,
    bold: bool,
}

impl Label {
    fn plain(text: String, color: Color) -> Self {
        Self {
            parts: vec![(text, color)],
            bold: false,
        }
    }

    fn width(&self) -> usize {
        self.parts.iter().map(|(text, _)| visual_width(text)).sum()
    }
}

struct PlacedLabel {
    x: usize,
    label: Label,
}

struct StatusPlan {
    line_len: usize,
    anim_end: usize,
    labels: Vec<PlacedLabel>,
    ralph_mode: bool,
    compact_activity: bool,
}

/// Keep the weekly percentage at the right edge, shortening the model first.
pub(super) fn draw_status_bar(
    buf: &mut Buffer,
    app: &AppState,
    size: TerminalSize,
    input_top: usize,
) {
    if input_top <= 1 || size.width == 0 {
        return;
    }
    let y = input_top - 2;
    let plan = status_plan(app, size);
    let color = if app.rewind_mode() {
        COLOR_DIFF_REMOVE
    } else {
        COLOR_STATUS_SEPARATOR
    };
    draw_str(
        buf,
        0,
        y,
        &"━".repeat(plan.anim_end),
        Style::default().fg(color),
        plan.anim_end,
    );
    draw_labels(buf, y, &plan.labels);
    if app.active_turn_id.is_some() && !app.rewind_mode() {
        let width = if plan.compact_activity {
            plan.line_len
        } else {
            plan.anim_end
        };
        sweep_status_line(buf, y, width, plan.ralph_mode, animation_tick());
    }
}

fn status_plan(app: &AppState, size: TerminalSize) -> StatusPlan {
    let line_len = size.width.saturating_sub(1);
    let mut edge = line_len;
    let mut labels = Vec::new();
    if let Some(weekly) = app.rate_limits.indicator() {
        place(
            &mut labels,
            &mut edge,
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
    place_context(&mut labels, &mut edge, app);
    place_model_and_activity(&mut labels, &mut edge, app);
    StatusPlan {
        line_len,
        anim_end: edge,
        labels,
        ralph_mode: app.ralph_enabled(),
        compact_activity: app.layout.uses_flow(size) || size.is_small() || edge < 8,
    }
}

fn place_context(labels: &mut Vec<PlacedLabel>, edge: &mut usize, app: &AppState) {
    let context = app
        .context_usage
        .map(context_usage_label)
        .unwrap_or_else(|| context_usage_placeholder_label().to_string());
    let (tokens, percent) = context.split_once(" (").unwrap_or((&context, ""));
    let percent = percent.trim_end_matches(')');
    let color = app
        .context_usage
        .map_or(COLOR_GUTTER_USER, |u| context_color(u.used, u.max));
    let full = Label {
        parts: vec![
            (format!("{tokens} "), COLOR_TOOL_TEXT),
            (percent.to_owned(), color),
        ],
        bold: false,
    };
    if !place(labels, edge, full) {
        place(labels, edge, Label::plain(percent.to_owned(), color));
    }
}

fn place_model_and_activity(labels: &mut Vec<PlacedLabel>, edge: &mut usize, app: &AppState) {
    let mut badges = Vec::new();
    if let Some(text) = app.dictation_status_label() {
        badges.push(Label {
            parts: vec![(text, COLOR_GUTTER_AGENT_COMMENTARY)],
            bold: true,
        });
    }
    if app.ralph_enabled() {
        badges.push(Label {
            parts: vec![("RALPH MODE".to_string(), COLOR_GUTTER_AGENT_THINKING)],
            bold: true,
        });
    }
    let reserved: usize = badges.iter().map(|label| label.width() + 1).sum();
    let budget = edge.saturating_sub(reserved + 2);
    let model = fit_model(&app.runtime_settings_label(), budget);
    place(labels, edge, Label::plain(model, COLOR_STEP8));
    for badge in badges {
        place(labels, edge, badge);
    }
}

fn fit_model(text: &str, width: usize) -> String {
    if visual_width(text) <= width {
        text.to_string()
    } else if width > 0 {
        format!("{}…", &text[..split_at_cells(text, width - 1)])
    } else {
        String::new()
    }
}

fn place(labels: &mut Vec<PlacedLabel>, edge: &mut usize, label: Label) -> bool {
    let width = label.width();
    if width == 0 || width + 2 > *edge {
        return false;
    }
    let x = *edge - width;
    *edge = x - 1;
    labels.push(PlacedLabel { x, label });
    true
}

fn draw_labels(buf: &mut Buffer, y: usize, labels: &[PlacedLabel]) {
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

fn sweep_status_line(buf: &mut Buffer, y: usize, width: usize, ralph_mode: bool, tick: u128) {
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
