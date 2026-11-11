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
    draw_claude_rewind_hint(buf, app, y, plan.anim_end);
    if (app.active_turn_id.is_some()
        || app.claude.activity.state == super::claude_activity::NativeState::Running
        || app.claude.tasks.active_count() > 0)
        && !app.rewind_mode()
    {
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
    if app.codex.enabled && app.codex.closed {
        let text = if app.codex.connection.rebuilding {
            "Codex reconnecting…"
        } else {
            "Codex offline · F5 reconnect"
        };
        place(
            &mut labels,
            &mut edge,
            Label::plain(text.into(), COLOR_GUTTER_USER),
        );
    }
    if let Some(label) = super::claude_edit::label(app, size.width) {
        place(
            &mut labels,
            &mut edge,
            Label::plain(label.to_string(), COLOR_GUTTER_USER),
        );
    }
    place_codex_callback(&mut labels, &mut edge, app, size);
    place_weekly(&mut labels, &mut edge, app);
    let mode = super::input_mode::label(app, std::time::Instant::now());
    let notice = queue_notice(app, size);
    let reserve = mode.map_or(0, |mode| visual_width(mode) + 1)
        + notice.as_ref().map_or(0, |notice| visual_width(notice) + 1);
    place_context(&mut labels, &mut edge, app, reserve);
    place_input_mode(&mut labels, &mut edge, mode, notice);
    place_tasks(&mut labels, &mut edge, app);
    place_model_and_activity(&mut labels, &mut edge, app);
    StatusPlan {
        line_len,
        anim_end: edge,
        labels,
        ralph_mode: app.ralph_enabled(),
        compact_activity: app.layout.uses_flow(size) || size.is_small() || edge < 8,
    }
}

fn place_weekly(labels: &mut Vec<PlacedLabel>, edge: &mut usize, app: &AppState) {
    if let Some(weekly) = if app.claude.enabled {
        app.claude.quota.indicator()
    } else {
        app.rate_limits.indicator()
    } {
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
}

fn place_tasks(labels: &mut Vec<PlacedLabel>, edge: &mut usize, app: &AppState) {
    let tasks = super::task_badge::count(app);
    let uncertain = app.codex.enabled
        && !app.codex.closed
        && app
            .codex
            .task_status
            .errors
            .keys()
            .any(|owner| super::task_badge::owns(app, owner));
    if tasks > 0 || uncertain {
        let hint = if tasks == 0 {
            "Tasks?".to_string()
        } else {
            format!(
                "{tasks}{} {}",
                if uncertain { "+" } else { "" },
                if tasks == 1 { "task" } else { "tasks" }
            )
        };
        place(labels, edge, Label::plain(hint, COLOR_GUTTER_USER));
    }
}

fn place_codex_callback(
    labels: &mut Vec<PlacedLabel>,
    edge: &mut usize,
    app: &AppState,
    size: TerminalSize,
) {
    if app.codex.enabled && !app.codex.callbacks.focused && !app.codex.callbacks.entries.is_empty()
    {
        let count = app.codex.callbacks.entries.len();
        let hint = if size.width >= 60 {
            format!("F4: Codex request waiting ({count})")
        } else if size.width >= 30 {
            format!("F4: request waiting ({count})")
        } else if size.width >= 16 {
            format!("F4 request ({count})")
        } else {
            "F4".into()
        };
        place(labels, edge, Label::plain(hint, COLOR_GUTTER_USER));
    }
}

fn queue_notice(app: &AppState, size: TerminalSize) -> Option<String> {
    if !app.codex.enabled || app.rewind_mode() {
        return None;
    }
    let compact = size.width < 32;
    if app.codex.queue.operation.is_some() {
        Some(if compact { "Q…" } else { "Queue pending" }.into())
    } else if app.codex.queue.recovery.is_some() {
        Some(if compact { "Draft" } else { "Unsent draft" }.into())
    } else if app.codex.queue.error.is_some() {
        Some(if compact { "Q!" } else { "Queue error" }.into())
    } else if app.codex.queue.known && !app.codex.queue.entries.is_empty() {
        Some(format!("{} queued", app.codex.queue.entries.len()))
    } else {
        None
    }
}

fn place_input_mode(
    labels: &mut Vec<PlacedLabel>,
    edge: &mut usize,
    mode: Option<&str>,
    notice: Option<String>,
) {
    if let Some(notice) = notice {
        place(labels, edge, Label::plain(notice, COLOR_GUTTER_USER));
    }
    if let Some(mode) = mode {
        place(
            labels,
            edge,
            Label::plain(mode.to_string(), COLOR_GUTTER_USER),
        );
    }
}

fn place_context(labels: &mut Vec<PlacedLabel>, edge: &mut usize, app: &AppState, reserve: usize) {
    let context = app
        .context_usage
        .map(context_usage_label)
        .unwrap_or_else(|| context_usage_placeholder_label().to_string());
    let stale = app.claude.enabled && app.context_usage.is_some() && !app.claude.context.fresh;
    let context = if stale {
        format!("~{context}")
    } else {
        context
    };
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

fn place_model_and_activity(labels: &mut Vec<PlacedLabel>, edge: &mut usize, app: &AppState) {
    let mut badges = Vec::new();
    if app.claude.enabled {
        if let Some(text) = app.claude.activity.label() {
            badges.push(Label::plain(text, COLOR_GUTTER_AGENT_THINKING));
        }
    }
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
    let label = if app.codex.enabled {
        super::codex_settings::label(app)
    } else {
        app.runtime_settings_label()
    };
    let model = fit_model(&label, budget);
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

fn draw_claude_rewind_hint(buf: &mut Buffer, app: &AppState, y: usize, width: usize) {
    if !app.rewind_mode() || app.claude.history.unknown.is_empty() {
        return;
    }
    let hint = if app.claude.history.unknown_selection.is_some() {
        "Unknown source: Enter branches; F4 human"
    } else {
        "F4: older unknown prompts"
    };
    draw_str(
        buf,
        0,
        y,
        hint,
        Style::default().fg(COLOR_DIFF_REMOVE),
        width,
    );
}
