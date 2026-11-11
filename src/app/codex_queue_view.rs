//! Edit native queued messages in a separate panel with a pinned selected identity.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::style::Style;

use super::codex_queue::{begin_edit, open, operate, save_edit};
use super::input_events::TerminalEventResult;
use super::render::{draw_str, fill_rect, textarea_input_from_key};
use super::state::AppState;
use super::text::{split_at_cells, visual_width, wrap_natural_by_cells};
use super::TerminalSize;
use crate::backend::BackendClient;
use crate::theme::{COLOR_DIM, COLOR_OVERLAY, COLOR_PRIMARY, COLOR_TEXT};

/// Keep queue keys separate from textarea newline keys and independent callbacks.
pub(super) fn key(
    client: &dyn BackendClient,
    app: &mut AppState,
    key: KeyEvent,
    size: TerminalSize,
) -> Option<TerminalEventResult> {
    if !app.codex.enabled {
        return None;
    }
    if key.code == KeyCode::F(3) && key.modifiers.is_empty() {
        super::input_mode::toggle(app, std::time::Instant::now());
        return Some(TerminalEventResult::Continue { needs_draw: true });
    }
    if !app.codex.queue.show {
        if key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::ALT) {
            open(app);
            return Some(TerminalEventResult::Continue { needs_draw: true });
        }
        return None;
    }
    if key.code == KeyCode::Esc {
        if app.codex.queue.edit.is_some() {
            app.codex.queue.edit = None;
        } else {
            app.codex.queue.show = false;
        }
    } else if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Some(TerminalEventResult::Quit);
    } else if app.codex.queue.edit.is_some() {
        if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
            save_edit(client, app);
        } else if editable(size) {
            app.codex
                .queue
                .edit
                .as_mut()
                .expect("queue editor")
                .editor
                .input(textarea_input_from_key(key));
        }
    } else {
        list_key(client, app, key);
    }
    Some(TerminalEventResult::Continue { needs_draw: true })
}

fn list_key(client: &dyn BackendClient, app: &mut AppState, key: KeyEvent) {
    match (key.code, key.modifiers) {
        (KeyCode::Up, KeyModifiers::ALT) => operate(client, app, "up"),
        (KeyCode::Down, KeyModifiers::ALT) => operate(client, app, "down"),
        (KeyCode::Up, _) => app.codex.queue.move_selection(-1),
        (KeyCode::Down, _) => app.codex.queue.move_selection(1),
        (KeyCode::Char('e'), _) => begin_edit(app),
        (KeyCode::Char('d'), _) => operate(client, app, "delete"),
        (KeyCode::Char('s'), _) => operate(client, app, "start"),
        (KeyCode::Char('r'), _) => open(app),
        (KeyCode::Char('v'), _) => {
            if app.input_is_empty() {
                if let Some(text) = app.codex.queue.recovery.take() {
                    app.set_input_text(&text);
                }
                app.codex.queue.show = false;
            } else {
                super::codex_queue::fail(
                    app,
                    "Clear or save the main draft before recovering another",
                );
            }
        }
        _ => {}
    }
}

/// Consume queue paste without leaking it into the main composer or a hidden editor.
pub(super) fn paste(app: &mut AppState, text: &str, size: TerminalSize) {
    if editable(size) {
        if let Some(edit) = app.codex.queue.edit.as_mut() {
            edit.editor.insert_str(text);
        }
    }
}

/// Show selected identity before entries, so external changes cannot hide a stop/start target.
pub(super) fn draw(buf: &mut Buffer, app: &AppState, size: TerminalSize) {
    fill_rect(
        buf,
        0,
        0,
        size.width,
        size.height,
        Style::default().bg(COLOR_OVERLAY),
    );
    let width = size.width.saturating_sub(2).max(1);
    let queue = &app.codex.queue;
    let lines = body_lines(app, size);
    let footer = footer_lines(app, size);
    let body_height = size
        .height
        .saturating_sub(footer.len() + 2 + usize::from(queue.edit.is_some()));
    let body = lines
        .into_iter()
        .flat_map(|line| {
            wrap_natural_by_cells(&super::claude_question_state::safe_text(&line), width)
        })
        .collect::<Vec<_>>();
    for (row, line) in body.iter().take(body_height).enumerate() {
        draw_str(
            buf,
            1,
            row,
            line,
            Style::default().fg(if row == 0 { COLOR_PRIMARY } else { COLOR_TEXT }),
            width,
        );
    }
    if let Some((text, _)) = editor_line(app, width) {
        draw_str(
            buf,
            1,
            body_height,
            &text,
            Style::default().fg(COLOR_TEXT),
            width,
        );
    }
    for (index, line) in footer.iter().take(size.height).enumerate() {
        draw_str(
            buf,
            1,
            size.height.saturating_sub(footer.len()) + index,
            line,
            Style::default().fg(COLOR_DIM),
            width,
        );
    }
}

fn body_lines(app: &AppState, size: TerminalSize) -> Vec<String> {
    let queue = &app.codex.queue;
    let mut lines = vec!["Codex queue · Esc back".to_string()];
    if !editable(size) {
        lines.push("Resize to edit".into());
    }
    let state = if queue.read.is_some() {
        "Refreshing"
    } else if queue.known {
        "Current"
    } else {
        "Unknown"
    };
    lines.push(format!("{state}: {} queued", queue.entries.len()));
    let position = queue
        .entries
        .iter()
        .position(|entry| entry["id"].as_str() == queue.selected.as_deref());
    lines.push(format!(
        "Selected {}/{}: {}",
        position.map_or(0, |index| index + 1),
        queue.entries.len(),
        queue.selected.as_deref().unwrap_or("none")
    ));
    if let Some(error) = &queue.error {
        lines.push(error.clone());
    } else if !queue.notice.is_empty() {
        lines.push(queue.notice.clone());
    }
    if queue.selected.is_some() && queue.selected_entry().is_none() {
        lines.push("Selected entry left the queue; arrows choose another".into());
    }
    if let Some(entry) = queue.selected_entry() {
        lines.push(format!(
            "Client: {}",
            entry["clientUserMessageId"].as_str().unwrap_or("unknown")
        ));
        let text = entry["input"]
            .as_array()
            .map(|input| {
                input
                    .iter()
                    .map(|part| {
                        part["text"]
                            .as_str()
                            .map(str::to_string)
                            .unwrap_or_else(|| part.to_string())
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        lines.push(preview(&text));
    }
    lines
}

/// Keep the independent edit caret visible after horizontal scrolling and resizing.
pub(super) fn cursor(app: &AppState, size: TerminalSize) -> (usize, usize) {
    if !editable(size) {
        return (0, 0);
    }
    let width = size.width.saturating_sub(2).max(1);
    let footer_rows = footer_lines(app, size).len();
    editor_line(app, width).map_or((0, 0), |(_, column)| {
        (column + 1, size.height.saturating_sub(footer_rows + 3))
    })
}

fn editor_line(app: &AppState, width: usize) -> Option<(String, usize)> {
    let editor = &app.codex.queue.edit.as_ref()?.editor;
    let (row, column) = editor.cursor();
    let line = &editor.lines()[row];
    let prefix = line.chars().take(column).collect::<String>();
    let cells = visual_width(&prefix);
    let skip = cells.saturating_sub(width.saturating_sub(1));
    let start = split_at_cells(line, skip);
    Some((
        super::claude_question_state::safe_text(&line[start..]),
        cells.saturating_sub(skip),
    ))
}

fn footer_lines(app: &AppState, size: TerminalSize) -> Vec<String> {
    let text = if !editable(size) {
        "Esc back"
    } else if app.codex.queue.edit.is_some() {
        "Ctrl+S save · Esc back"
    } else if size.width < 60 {
        "↑↓ select e edit d delete s start Alt+↑↓ move r refresh v draft F3 mode Esc"
    } else {
        "↑↓ select · e edit · d delete · s start · Alt+↑↓ reorder · r refresh · v recover draft · F3 mode · Esc back"
    };
    wrap_natural_by_cells(text, size.width.saturating_sub(2).max(1))
}

fn preview(text: &str) -> String {
    let mut shortened = text.chars().take(1024).collect::<String>();
    if shortened.len() < text.len() {
        shortened.push_str("\n[Preview only; full native input is retained]");
    }
    shortened
}

fn editable(size: TerminalSize) -> bool {
    size.width >= 22 && size.height >= 12
}
