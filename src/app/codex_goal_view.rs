//! Keep explicit goal controls and independent objective/budget edits visible on narrow terminals.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::style::Style;

use super::codex_goal_state::GoalConfirm;
use super::input_events::TerminalEventResult;
use super::render::{draw_str, fill_rect, textarea_input_from_key};
use super::state::AppState;
use super::text::{split_at_cells, visual_width};
use super::TerminalSize;
use crate::backend::BackendClient;
use crate::theme::{COLOR_DIM, COLOR_OVERLAY, COLOR_PRIMARY, COLOR_TEXT};

/// Keep goal keys separate from ordinary chat, question answers and queue drafts.
pub(super) fn key(
    client: &dyn BackendClient,
    app: &mut AppState,
    key: KeyEvent,
    size: TerminalSize,
) -> Option<TerminalEventResult> {
    if !app.codex.enabled {
        return None;
    }
    if !app.codex.goals.show {
        if key.code == KeyCode::Char('g') && key.modifiers.contains(KeyModifiers::ALT) {
            app.codex.goals.show = true;
            app.codex.goals.last_read = None;
        } else {
            return None;
        }
    } else if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Some(TerminalEventResult::Quit);
    } else if key.code == KeyCode::F(4) {
        app.codex.goals.show = false;
        super::codex_callbacks::next(app);
    } else if key.code == KeyCode::Char('r') && key.modifiers.contains(KeyModifiers::CONTROL) {
        if let Err(error) = app.request_ralph_toggle() {
            app.codex.goals.error = Some(error.to_string());
        }
    } else if key.code == KeyCode::Esc {
        if app.codex.goals.confirm.take().is_none() {
            if app.codex.goals.editing {
                app.codex.goals.editing = false;
            } else {
                app.codex.goals.show = false;
            }
        }
    } else if app.codex.goals.confirm.is_some() {
        match key.code {
            KeyCode::Char('y') => super::codex_goals::confirm(client, app),
            KeyCode::Char('n') => app.codex.goals.confirm = None,
            KeyCode::PageUp => app.codex.goals.scroll = app.codex.goals.scroll.saturating_sub(5),
            KeyCode::PageDown => app.codex.goals.scroll = app.codex.goals.scroll.saturating_add(5),
            _ => {}
        }
    } else if app.codex.goals.editing {
        editor_key(client, app, key, size);
    } else {
        overview_key(client, app, key);
    }
    Some(TerminalEventResult::Continue { needs_draw: true })
}

fn overview_key(client: &dyn BackendClient, app: &mut AppState, key: KeyEvent) {
    match key.code {
        KeyCode::Char('e') => super::codex_goals::edit(app),
        KeyCode::Char('p') => super::codex_goals::status(client, app, "paused"),
        KeyCode::Char('a') => super::codex_goals::status(client, app, "active"),
        KeyCode::Char('d') => {
            app.codex.goals.scroll = 0;
            app.codex.goals.confirm = Some(GoalConfirm::Clear);
        }
        KeyCode::Char('r') => app.codex.goals.last_read = None,
        KeyCode::PageUp => app.codex.goals.scroll = app.codex.goals.scroll.saturating_sub(5),
        KeyCode::PageDown => app.codex.goals.scroll = app.codex.goals.scroll.saturating_add(5),
        _ => {}
    }
}

fn editor_key(client: &dyn BackendClient, app: &mut AppState, key: KeyEvent, size: TerminalSize) {
    if app.codex.goals.mutation.is_some() {
        return;
    }
    if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
        if editable(size) {
            super::codex_goals::save(client, app);
        }
        return;
    }
    let draft = app.codex.goals.editor.as_mut().expect("goal editor");
    match key.code {
        KeyCode::Tab => draft.field = (draft.field + 1) % 3,
        KeyCode::BackTab => draft.field = (draft.field + 2) % 3,
        _ if !editable(size) => {}
        KeyCode::Left | KeyCode::Right if draft.field == 2 => {
            draft.status = Some(
                if key.code == KeyCode::Left {
                    "paused"
                } else {
                    "active"
                }
                .into(),
            );
        }
        _ if draft.field == 0 => {
            draft.objective.input(textarea_input_from_key(key));
        }
        _ if draft.field == 1 && key.code != KeyCode::Enter => {
            draft.budget.input(textarea_input_from_key(key));
        }
        _ => {}
    }
}

/// Consume goal paste even on the resize fallback or while a mutation locks its draft.
pub(super) fn paste(app: &mut AppState, text: &str, size: TerminalSize) {
    if !app.codex.goals.editing
        || !editable(size)
        || app.codex.goals.mutation.is_some()
        || app.codex.goals.confirm.is_some()
    {
        return;
    }
    if let Some(draft) = app.codex.goals.editor.as_mut() {
        if draft.field == 0 && draft.objective.lines().join("\n").len() + text.len() <= 32 * 1024 {
            draft.objective.insert_str(text);
        } else if draft.field == 1 && text.len() <= 32 {
            draft.budget.insert_str(text.lines().next().unwrap_or(""));
        }
    }
}

/// Draw native facts or one active editor field without covering the visible control footer.
pub(super) fn draw(buf: &mut Buffer, app: &AppState, size: TerminalSize) {
    fill_rect(
        buf,
        0,
        0,
        size.width,
        size.height,
        Style::default().bg(COLOR_OVERLAY),
    );
    if size.width == 0 || size.height == 0 {
        return;
    }
    draw_str(
        buf,
        0,
        0,
        "Codex goal · Esc back",
        Style::default().fg(COLOR_PRIMARY),
        size.width,
    );
    let footer = if app.codex.goals.confirm.is_some() {
        "y confirms · n cancels · Esc back"
    } else if app.codex.goals.editing {
        "Tab field · Ctrl+S save · Ctrl+R Ralph · Esc back"
    } else {
        "e edit · p pause · a resume · d clear · r refresh · PgDn · Esc back"
    };
    let footer = super::claude_question_view::wrap_lines(vec![footer.into()], size.width.max(1));
    let footer_rows = footer.len().min(size.height.saturating_sub(2));
    let body_height = size.height.saturating_sub(footer_rows + 1);
    let lines = super::codex_goal_text::body(app, size);
    let wrapped = super::claude_question_view::wrap_lines(lines, size.width.max(1));
    let offset = app
        .codex
        .goals
        .scroll
        .min(wrapped.len().saturating_sub(body_height));
    for (row, line) in wrapped.iter().skip(offset).take(body_height).enumerate() {
        draw_str(
            buf,
            0,
            row + 1,
            line,
            Style::default().fg(COLOR_TEXT),
            size.width,
        );
    }
    if app.codex.goals.editing && app.codex.goals.confirm.is_none() && editable(size) {
        draw_editor(buf, app, size, body_height);
    }
    for (row, line) in footer.iter().take(footer_rows).enumerate() {
        draw_str(
            buf,
            0,
            size.height - footer_rows + row,
            line,
            Style::default().fg(COLOR_DIM),
            size.width,
        );
    }
}

fn draw_editor(buf: &mut Buffer, app: &AppState, size: TerminalSize, height: usize) {
    if let Some((text, column)) = editor_line(app, size.width) {
        let _ = column;
        draw_str(
            buf,
            0,
            height,
            &" ".repeat(size.width),
            Style::default().bg(COLOR_OVERLAY),
            size.width,
        );
        draw_str(
            buf,
            0,
            height,
            &text,
            Style::default().fg(COLOR_TEXT),
            size.width,
        );
    }
}

/// Restore the independent goal caret after resizing; ordinary composer coordinates stay untouched.
pub(super) fn cursor(app: &AppState, size: TerminalSize) -> (usize, usize) {
    if !editable(size) || app.codex.goals.confirm.is_some() {
        return (0, 0);
    }
    let footer = super::claude_question_view::wrap_lines(
        vec!["Tab field · Ctrl+S save · Ctrl+R Ralph · Esc back".into()],
        size.width.max(1),
    );
    let y = size.height.saturating_sub(footer.len() + 1);
    editor_line(app, size.width).map_or((0, 0), |(_, column)| (column, y))
}

fn editor_line(app: &AppState, width: usize) -> Option<(String, usize)> {
    let draft = app.codex.goals.editor.as_ref()?;
    let editor = match draft.field {
        0 => &draft.objective,
        1 => &draft.budget,
        _ => return None,
    };
    let (row, column) = editor.cursor();
    let line = &editor.lines()[row];
    let before: String = line.chars().take(column).collect();
    let cells = visual_width(&before);
    let skip = cells.saturating_sub(width.saturating_sub(1));
    let start = split_at_cells(line, skip);
    Some((
        super::claude_question_state::safe_text(&line[start..]),
        cells.saturating_sub(skip),
    ))
}

fn editable(size: TerminalSize) -> bool {
    size.width >= 22 && size.height >= 12
}
