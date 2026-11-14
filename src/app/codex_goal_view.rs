//! Keep explicit goal controls and independent objective/budget edits visible on narrow terminals.

use carlos_core::backend::BackendClient;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::style::Style;

use super::codex_goal_state::{GoalConfirm, GoalDraft, GoalView};
use super::input_events::TerminalEventResult;
use super::render::{draw_str, fill_rect, textarea_input_from_key};
use super::state::AppState;
use super::text::{split_at_cells, visual_width};
use super::TerminalSize;
use crate::theme::{COLOR_DIM, COLOR_OVERLAY, COLOR_PRIMARY, COLOR_TEXT};

/// Keep goal keys separate from ordinary chat, question answers and queue drafts.
pub(super) fn key(
    client: &dyn BackendClient,
    app: &mut AppState,
    key: KeyEvent,
    size: TerminalSize,
) -> Option<TerminalEventResult> {
    if !app.session.codex_scope().enabled() {
        return None;
    }
    if !app.view.codex_goals.show {
        if key.code == KeyCode::Char('g') && key.modifiers.contains(KeyModifiers::ALT) {
            app.view.codex_goals.show = true;
            app.session.refresh_codex_goal();
        } else {
            return None;
        }
    } else if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Some(TerminalEventResult::Quit);
    } else if key.code == KeyCode::F(4) {
        app.view.codex_goals.show = false;
        super::codex_callbacks::next(app);
    } else if key.code == KeyCode::Char('r') && key.modifiers.contains(KeyModifiers::CONTROL) {
        if let Err(error) = app.request_ralph_toggle() {
            app.view.codex_goals.error = Some(error.to_string());
        }
    } else if key.code == KeyCode::Esc {
        if app.view.codex_goals.confirm.take().is_none() {
            if app.view.codex_goals.editing {
                app.view.codex_goals.editing = false;
            } else {
                app.view.codex_goals.show = false;
            }
        }
    } else if app.view.codex_goals.confirm.is_some() {
        match key.code {
            KeyCode::Char('y') => super::codex_goals::confirm(client, app),
            KeyCode::Char('n') => app.view.codex_goals.confirm = None,
            KeyCode::PageUp => {
                app.view.codex_goals.scroll = app.view.codex_goals.scroll.saturating_sub(5)
            }
            KeyCode::PageDown => {
                app.view.codex_goals.scroll = app.view.codex_goals.scroll.saturating_add(5)
            }
            _ => {}
        }
    } else if app.view.codex_goals.editing {
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
            app.view.codex_goals.scroll = 0;
            app.view.codex_goals.confirm = Some(GoalConfirm::Clear);
        }
        KeyCode::Char('r') => app.session.refresh_codex_goal(),
        KeyCode::PageUp => {
            app.view.codex_goals.scroll = app.view.codex_goals.scroll.saturating_sub(5)
        }
        KeyCode::PageDown => {
            app.view.codex_goals.scroll = app.view.codex_goals.scroll.saturating_add(5)
        }
        _ => {}
    }
}

fn editor_key(client: &dyn BackendClient, app: &mut AppState, key: KeyEvent, size: TerminalSize) {
    if app.session.codex_goals().pending() {
        return;
    }
    if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
        if editable(size) {
            super::codex_goals::save(client, app);
        }
        return;
    }
    let draft = app.view.codex_goals.editor.as_mut().expect("goal editor");
    edit_draft(draft, key, size);
}

/// Edit a local goal draft without borrowing shared session facts.
pub(super) fn edit_draft(draft: &mut GoalDraft, key: KeyEvent, size: TerminalSize) {
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
    if !app.view.codex_goals.editing
        || !editable(size)
        || app.session.codex_goals().pending()
        || app.view.codex_goals.confirm.is_some()
    {
        return;
    }
    if let Some(draft) = app.view.codex_goals.editor.as_mut() {
        paste_draft(draft, text);
    }
}

/// Insert pasted text into only the chosen goal field.
pub(super) fn paste_draft(draft: &mut GoalDraft, text: &str) {
    if draft.field == 0 && draft.objective.lines().join("\n").len() + text.len() <= 32 * 1024 {
        draft.objective.insert_str(text);
    } else if draft.field == 1 && text.len() <= 32 {
        draft.budget.insert_str(text.lines().next().unwrap_or(""));
    }
}

/// Draw native facts or one active editor field without covering the visible control footer.
pub(super) fn draw(buf: &mut Buffer, app: &AppState, size: TerminalSize) {
    draw_panel(
        buf,
        &app.view.codex_goals,
        super::codex_goal_text::body(app, size),
        size,
    );
}

/// Draw the same goal widget for embedded and daemon facts.
pub(super) fn draw_panel(
    buf: &mut Buffer,
    state: &GoalView,
    lines: Vec<String>,
    size: TerminalSize,
) {
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
    let footer = footer(state, size.width);
    let footer_rows = footer.len().min(size.height.saturating_sub(2));
    let body_height = size.height.saturating_sub(footer_rows + 1);
    let wrapped = super::claude_question_view::wrap_lines(lines, size.width.max(1));
    let offset = state.scroll.min(wrapped.len().saturating_sub(body_height));
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
    if state.editing && state.confirm.is_none() && editable(size) {
        draw_editor(buf, state, size, body_height);
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

fn draw_editor(buf: &mut Buffer, state: &GoalView, size: TerminalSize, height: usize) {
    if let Some((text, column)) = editor_line(state, size.width) {
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
    cursor_for(&app.view.codex_goals, size)
}

/// Locate the independent goal caret for either session transport.
pub(super) fn cursor_for(state: &GoalView, size: TerminalSize) -> (usize, usize) {
    if !editable(size) || state.confirm.is_some() {
        return (0, 0);
    }
    let footer = super::claude_question_view::wrap_lines(
        vec!["Tab field · Ctrl+S save · Ctrl+R Ralph · Esc back".into()],
        size.width.max(1),
    );
    let y = size.height.saturating_sub(footer.len() + 1);
    editor_line(state, size.width).map_or((0, 0), |(_, column)| (column, y))
}

fn editor_line(state: &GoalView, width: usize) -> Option<(String, usize)> {
    let draft = state.editor.as_ref()?;
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
        carlos_core::session::text::safe_text(&line[start..]),
        cells.saturating_sub(skip),
    ))
}

/// Report whether the terminal has room for the goal editor.
pub(super) fn editable(size: TerminalSize) -> bool {
    size.width >= 22 && size.height >= 12
}

fn footer(state: &GoalView, width: usize) -> Vec<String> {
    let footer = if state.confirm.is_some() {
        "y confirms · n cancels · Esc back"
    } else if state.editing {
        "Tab field · Ctrl+S save · Ctrl+R Ralph · Esc back"
    } else {
        "e edit · p pause · a resume · d clear · r refresh · PgDn · Esc back"
    };
    super::claude_question_view::wrap_lines(vec![footer.into()], width.max(1))
}
