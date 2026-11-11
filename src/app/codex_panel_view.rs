//! Draw and navigate Codex agents/processes without editing composer or callback drafts.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::style::Style;

use super::codex_panel_state::PanelId;
use super::input_events::TerminalEventResult;
use super::models::TerminalSize;
use super::render::{draw_str, fill_rect};
use super::state::AppState;
use crate::backend::{BackendClient, BackendKind};
use crate::theme::{COLOR_DIM, COLOR_PRIMARY, COLOR_STEP2, COLOR_TEXT};

/// Keep F2 independent of model/queue/question overlays; closing reveals their untouched drafts.
pub(super) fn key(
    client: &dyn BackendClient,
    app: &mut AppState,
    key: KeyEvent,
) -> Option<TerminalEventResult> {
    if client.kind() != BackendKind::Codex {
        return None;
    }
    if app.codex.panel.show && key.code == KeyCode::F(3) && key.modifiers.is_empty() {
        super::input_mode::toggle(app, std::time::Instant::now());
    } else if key.code == KeyCode::F(2) && key.modifiers.is_empty() {
        app.codex.panel.show = !app.codex.panel.show;
        if app.codex.panel.show {
            if app.codex.panel.selected.is_none() {
                app.codex.panel.selected = app.codex.panel.ids().first().cloned();
            }
            super::codex_panel::refresh(app, false);
        }
    } else if app.codex.panel.show {
        match (key.code, key.modifiers) {
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => return Some(TerminalEventResult::Quit),
            (KeyCode::F(4), _) => {
                app.codex.panel.show = false;
                super::codex_callbacks::next(app);
            }
            (KeyCode::Esc, _) if app.codex.panel.detail => {
                app.codex.panel.detail = false;
                app.codex.panel.scroll = 0;
            }
            (KeyCode::Esc, _) => app.codex.panel.show = false,
            (KeyCode::Up, _) => app.codex.panel.select(false),
            (KeyCode::Down, _) => app.codex.panel.select(true),
            (KeyCode::PageUp, _) => {
                app.codex.panel.scroll = app.codex.panel.scroll.saturating_sub(5)
            }
            (KeyCode::PageDown, _) => {
                app.codex.panel.scroll = app.codex.panel.scroll.saturating_add(5)
            }
            (KeyCode::Enter, _) => super::codex_panel::details(app, false),
            (KeyCode::Char('l'), KeyModifiers::NONE) => {
                if app.codex.panel.detail {
                    super::codex_panel::details(app, true);
                } else {
                    super::codex_panel::refresh(app, true);
                }
            }
            (KeyCode::Char('r'), KeyModifiers::NONE) => {
                app.codex.panel.error = None;
                if app.codex.panel.detail {
                    super::codex_panel::details(app, false);
                } else {
                    super::codex_panel::refresh(app, false);
                }
            }
            (KeyCode::Char('x'), KeyModifiers::NONE) => super::codex_panel::stop(app),
            _ => {}
        }
    } else {
        return None;
    }
    Some(TerminalEventResult::Continue { needs_draw: true })
}

/// Pin the selected native row above the scrollable details even on a narrow terminal.
pub(super) fn draw(buf: &mut Buffer, app: &AppState, size: TerminalSize) {
    fill_rect(
        buf,
        0,
        0,
        size.width,
        size.height,
        Style::default().bg(COLOR_STEP2),
    );
    if size.width == 0 || size.height == 0 {
        return;
    }
    draw_str(
        buf,
        0,
        0,
        "Codex agents / processes",
        Style::default().fg(COLOR_PRIMARY),
        size.width,
    );
    if size.height < 3 {
        return;
    }
    draw_str(
        buf,
        0,
        1,
        &selected(app),
        Style::default().fg(COLOR_PRIMARY),
        size.width,
    );
    let lines = super::codex_panel_text::body(app);
    let wrapped = super::claude_question_view::wrap_lines(lines, size.width.max(1));
    let compact = size.width < 40 && size.height >= 4;
    let height = size.height.saturating_sub(if compact { 4 } else { 3 });
    let offset = app
        .codex
        .panel
        .scroll
        .min(wrapped.len().saturating_sub(height));
    for (row, line) in wrapped.iter().skip(offset).take(height).enumerate() {
        draw_str(
            buf,
            0,
            row + 2,
            line,
            Style::default().fg(COLOR_TEXT),
            size.width,
        );
    }
    if compact {
        draw_str(
            buf,
            0,
            size.height - 2,
            "↑↓ Enter PgDn",
            Style::default().fg(COLOR_DIM),
            size.width,
        );
    }
    let footer = if size.width < 40 {
        "x stop F2 back"
    } else {
        "↑↓ select · Enter details · x stop · r latest · l older · PgDn · F3 mode · F4 questions · F2 back"
    };
    draw_str(
        buf,
        0,
        size.height - 1,
        footer,
        Style::default().fg(COLOR_DIM),
        size.width,
    );
}

fn selected(app: &AppState) -> String {
    let panel = &app.codex.panel;
    if !panel.selected_exists() {
        return if panel.selected.is_some() {
            "Selected row unavailable; use arrows"
        } else {
            "No selected agent/process"
        }
        .into();
    }
    let (label, status) = match panel.selected.as_ref().expect("selected row") {
        PanelId::Agent(id) => {
            let agent = panel
                .agents
                .iter()
                .find(|agent| agent["id"] == *id)
                .expect("selected agent");
            (
                agent["agentNickname"].as_str().unwrap_or(id),
                agent["status"]["type"].as_str().unwrap_or("unknown"),
            )
        }
        PanelId::Process { process, .. } => (process.as_str(), "background process"),
    };
    super::claude_question_state::safe_label(&format!("> {label}: {status}"))
}
