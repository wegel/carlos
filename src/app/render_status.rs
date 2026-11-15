//! Fit context and weekly quota labels beside the animated status separator.

use ratatui::buffer::Buffer;
use ratatui::style::Style;

use super::notifications::animation_tick;

use super::render::{draw_status_separator, draw_str};
use super::state::AppState;
use super::status_line::{draw_labels, fit_model, place, sweep_status_line, Label, PlacedLabel};
use super::text::visual_width;

use super::view_models::TerminalSize;
use super::weekly_usage::WeeklyIndicator;
use crate::theme::{
    COLOR_DIFF_REMOVE, COLOR_GUTTER_AGENT_COMMENTARY, COLOR_GUTTER_AGENT_THINKING,
    COLOR_GUTTER_USER, COLOR_STEP8,
};

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
    draw_status_separator(buf, y, plan.anim_end, app.rewind_mode());
    draw_labels(buf, y, &plan.labels);
    draw_claude_rewind_hint(buf, app, y, plan.anim_end);
    if (app.session.active_turn_id().is_some()
        || app.session.claude().activity().is_running()
        || app.session.claude().tasks().active_count() > 0)
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
    if app.session.codex_scope().enabled() && app.session.codex_scope().closed() {
        let text = if app.session.codex_connection().rebuilding() {
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
    if let Some(label) = super::claude_edit_view::label(app, size.width) {
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
        compact_activity: app.view.layout.uses_flow(size) || size.is_small() || edge < 8,
    }
}

fn place_weekly(labels: &mut Vec<PlacedLabel>, edge: &mut usize, app: &AppState) {
    if let Some(weekly) = if app.session.claude().enabled() {
        app.session
            .claude()
            .quota()
            .indicator()
            .map(WeeklyIndicator::from)
    } else {
        app.session
            .rate_limits()
            .indicator()
            .map(WeeklyIndicator::from)
    } {
        super::status_line::place_weekly(labels, edge, weekly);
    }
}

fn place_tasks(labels: &mut Vec<PlacedLabel>, edge: &mut usize, app: &AppState) {
    let tasks = super::task_badge::count(app);
    let uncertain = app.session.codex_scope().enabled()
        && !app.session.codex_scope().closed()
        && app
            .session
            .codex_task_status()
            .errors()
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
    if app.session.codex_scope().enabled()
        && !app.view.codex_callbacks.focused
        && !app.session.codex_callbacks().is_empty()
    {
        let count = app.session.codex_callbacks().len();
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
    if !app.session.codex_scope().enabled() || app.rewind_mode() {
        return None;
    }
    let compact = size.width < 32;
    if app.session.codex_queue().pending() {
        Some(if compact { "Q…" } else { "Queue pending" }.into())
    } else if app.view.codex_queue.recovery.is_some() {
        Some(if compact { "Draft" } else { "Unsent draft" }.into())
    } else if super::codex_queue::error(app).is_some() {
        Some(if compact { "Q!" } else { "Queue error" }.into())
    } else if app.session.codex_queue().known() && !app.session.codex_queue().entries().is_empty() {
        Some(format!(
            "{} queued",
            app.session.codex_queue().entries().len()
        ))
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
    super::status_line::place_context(
        labels,
        edge,
        app.session.context_usage(),
        app.session.claude().enabled() && !app.session.claude().context_fresh(),
        reserve,
    );
}

fn place_model_and_activity(labels: &mut Vec<PlacedLabel>, edge: &mut usize, app: &AppState) {
    let mut badges = Vec::new();
    if app.session.claude().enabled() {
        if let Some(text) = super::claude_activity::label(app.session.claude().activity()) {
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
    let label = if app.session.codex_scope().enabled() {
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

fn draw_claude_rewind_hint(buf: &mut Buffer, app: &AppState, y: usize, width: usize) {
    if !app.rewind_mode() || app.session.claude_history().unknown().is_empty() {
        return;
    }
    let hint = if app.view.claude_unknown_selection.is_some() {
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
