//! Shows native tasks and quota windows while leaving composer and callback drafts untouched.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::style::{Modifier, Style};
use serde_json::json;

use super::claude_question_state::safe_label;
use super::claude_task_stops::StopState;
use super::input_events::TerminalEventResult;
use super::models::TerminalSize;
use super::render::{draw_str, fill_rect};
use super::state::AppState;
use crate::backend::{BackendClient, BackendKind};
use crate::theme::{COLOR_DIM, COLOR_PRIMARY, COLOR_STEP2, COLOR_TEXT};

/// Route task controls ahead of question input so a waiting child can still be stopped.
pub(super) fn key(
    client: &dyn BackendClient,
    app: &mut AppState,
    key: KeyEvent,
) -> Option<TerminalEventResult> {
    if client.kind() != BackendKind::Claude {
        return None;
    }
    if key.code == KeyCode::F(2) && key.modifiers.is_empty() {
        if app.claude.tasks.show {
            app.claude.tasks.show = false;
        } else {
            app.claude.tasks.open();
        }
    } else if app.claude.tasks.show {
        match (key.code, key.modifiers) {
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => return Some(TerminalEventResult::Quit),
            (KeyCode::Esc, _) => app.claude.tasks.show = false,
            (KeyCode::Up, _) => select(app, false),
            (KeyCode::Down, _) => select(app, true),
            (KeyCode::PageUp, _) => {
                app.claude.tasks.scroll = app.claude.tasks.scroll.saturating_sub(5)
            }
            (KeyCode::PageDown, _) => {
                app.claude.tasks.scroll = app.claude.tasks.scroll.saturating_add(5)
            }
            (KeyCode::Char('x'), KeyModifiers::NONE) => stop(client, app),
            _ => {}
        }
    } else {
        return None;
    }
    Some(TerminalEventResult::Continue { needs_draw: true })
}

/// Draw a selected task with wrapping and scrolling even on a phone-sized terminal.
pub(super) fn draw(buf: &mut Buffer, app: &AppState, size: TerminalSize) {
    fill_rect(
        buf,
        0,
        0,
        size.width,
        size.height,
        Style::default().bg(COLOR_STEP2),
    );
    if size.height == 0 || size.width == 0 {
        return;
    }
    draw_header(buf, app, size);
    let lines = body(app);
    let wrapped = super::claude_question_view::wrap_lines(lines, size.width.max(1));
    let height = size.height.saturating_sub(2);
    let offset = app
        .claude
        .tasks
        .scroll
        .min(wrapped.len().saturating_sub(height));
    for (row, line) in wrapped.iter().skip(offset).take(height).enumerate() {
        draw_str(
            buf,
            0,
            row + 1,
            line,
            Style::default().fg(COLOR_TEXT),
            size.width,
        );
    }
    draw_footer(buf, app, size);
}

fn detail(app: &AppState, id: Option<&str>) -> Vec<String> {
    let Some(id) = id else {
        return vec![if app.claude.tasks.selected.is_some() {
            "Selected task left the retained list; use arrows to choose another".to_string()
        } else {
            if app.claude.tasks.ids().is_empty() {
                "No native tasks reported".to_string()
            } else {
                "Use arrows to select a native task".to_string()
            }
        }];
    };
    let Some(data) = app.claude.tasks.data(id) else {
        return Vec::new();
    };
    let status = app
        .claude
        .tasks
        .records
        .get(id)
        .map(|record| record.status.label())
        .unwrap_or("running");
    let mut lines = vec![format!("Task {} · {status}", safe_label(id))];
    if let Some(state) = app.claude.tasks.stops.state(id, data["run_id"].as_str()) {
        lines.push(match state {
            StopState::Sending => "Sending stop request".to_string(),
            StopState::Requested => "Stop requested; waiting for native task event".to_string(),
            StopState::Failed(error) => format!("Stop failed: {}", safe_label(error)),
        });
    }
    for (field, label) in [
        ("description", "Description"),
        ("summary", "Summary"),
        ("run_id", "Run"),
        ("task_type", "Type"),
        ("parent_task_id", "Parent"),
        ("parent_tool_use_id", "Caller"),
        ("subagent_type", "Agent"),
        ("output_file", "Output"),
        ("error", "Error"),
    ] {
        if let Some(value) = data[field].as_str().filter(|value| !value.is_empty()) {
            lines.push(format!("{label}: {}", safe_label(value)));
        }
    }
    if let Some(usage) = data["usage"].as_object() {
        for field in ["total_tokens", "tool_uses", "duration_ms"] {
            if let Some(value) = usage.get(field) {
                lines.push(format!("{}: {value}", field.replace('_', " ")));
            }
        }
    }
    lines
}

fn select(app: &mut AppState, down: bool) {
    let ids = app.claude.tasks.ids();
    let current = app
        .claude
        .tasks
        .selected_id()
        .and_then(|id| ids.iter().position(|candidate| candidate == id));
    let next = current.map_or(0, |index| {
        if down {
            (index + 1).min(ids.len().saturating_sub(1))
        } else {
            index.saturating_sub(1)
        }
    });
    app.claude.tasks.selected = ids.get(next).cloned();
    app.claude.tasks.scroll = 0;
    app.claude.tasks.control_error = None;
}

fn stop(client: &dyn BackendClient, app: &mut AppState) {
    let Some(id) = app.claude.tasks.selected_id().map(str::to_string) else {
        return;
    };
    let Some(run) = app.claude.tasks.begin_stop(&id) else {
        return;
    };
    if let Err(error) = client.request_native_control("task.stop", json!({"taskId":id,"runId":run}))
    {
        app.claude
            .tasks
            .stop_reply(&id, run.as_deref(), Some(error.to_string()));
    }
}

fn draw_header(buf: &mut Buffer, app: &AppState, size: TerminalSize) {
    let ids = app.claude.tasks.ids();
    let selected = app.claude.tasks.selected_id();
    let index = selected
        .and_then(|id| ids.iter().position(|candidate| candidate == id))
        .map_or(0, |index| index + 1);
    let title = format!(
        "Tasks {index}/{} · {} active",
        ids.len(),
        app.claude.tasks.active_count()
    );
    draw_str(
        buf,
        0,
        0,
        &title,
        Style::default()
            .fg(COLOR_PRIMARY)
            .add_modifier(Modifier::BOLD),
        size.width,
    );
}

fn body(app: &AppState) -> Vec<String> {
    let selected = app.claude.tasks.selected_id();
    let mut lines = detail(app, selected);
    if let Some(error) = &app.claude.tasks.control_error {
        lines.push(safe_label(error));
    }
    if let Some(label) = app.claude.activity.label() {
        lines.push(format!("Main: {label}"));
    }
    for (id, prompt) in &app.claude.prompts.entries {
        if !prompt.clear {
            lines.push(format!(
                "Prompt {} · {} · {}",
                safe_label(id),
                if prompt.consumed {
                    "running"
                } else {
                    "accepted"
                },
                if prompt.priority == "now" {
                    "steer"
                } else {
                    "queued"
                }
            ));
        }
    }
    for window in app
        .claude
        .quota
        .windows
        .values()
        .filter(|window| window.reset > super::weekly_usage::unix_now())
    {
        lines.push(format!(
            "Quota {}: {:.1}% used; resets in {:.1}h",
            safe_label(&window.label),
            window.used,
            (window.reset - super::weekly_usage::unix_now()) as f64 / 3600.0
        ));
    }
    lines
}

fn draw_footer(buf: &mut Buffer, app: &AppState, size: TerminalSize) {
    draw_str(
        buf,
        0,
        size.height - 1,
        if size.width < 40 {
            if app
                .claude
                .tasks
                .selected_id()
                .is_some_and(|id| app.claude.tasks.can_stop(id))
            {
                "↑↓ x stop F2 back"
            } else {
                "↑↓ no stop F2 back"
            }
        } else {
            if app
                .claude
                .tasks
                .selected_id()
                .is_some_and(|id| app.claude.tasks.can_stop(id))
            {
                "↑↓ tasks · x stop · PgDn · F2/Esc close"
            } else {
                "↑↓ tasks · stop unavailable · PgDn · F2/Esc close"
            }
        },
        Style::default().fg(COLOR_DIM),
        size.width,
    );
}
