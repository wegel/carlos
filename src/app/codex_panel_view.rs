//! Draw and navigate Codex agents/processes without editing composer or callback drafts.

use carlos_core::backend::{BackendClient, BackendKind};
use carlos_core::session::codex::panel_state::PanelId;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::style::Style;

use super::input_events::TerminalEventResult;
use super::state::AppState;
use super::task_panel_rows::PanelRows;
use super::task_panel_view::{draw as draw_panel, TaskPanel};
use super::view_models::TerminalSize;
use crate::theme::COLOR_PRIMARY;

/// Keep F2 independent of model/queue/question overlays; closing reveals their untouched drafts.
pub(super) fn key(
    client: &dyn BackendClient,
    app: &mut AppState,
    key: KeyEvent,
) -> Option<TerminalEventResult> {
    if client.kind() != BackendKind::Codex {
        return None;
    }
    if app.view.codex_panel.show && key.code == KeyCode::F(3) && key.modifiers.is_empty() {
        super::input_mode::toggle(app, std::time::Instant::now());
    } else if key.code == KeyCode::F(2) && key.modifiers.is_empty() {
        toggle_panel(app);
    } else if app.view.codex_panel.show {
        match (key.code, key.modifiers) {
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => return Some(TerminalEventResult::Quit),
            (KeyCode::F(4), _) => {
                app.view.codex_panel.show = false;
                super::codex_callbacks::next(app);
            }
            (KeyCode::Esc, _) if app.view.codex_panel.detail => {
                app.view.codex_panel.detail = false;
                app.view.codex_panel.scroll = 0;
            }
            (KeyCode::Esc, _) => app.view.codex_panel.show = false,
            (KeyCode::Up, _) => super::codex_panel::select(app, false),
            (KeyCode::Down, _) => super::codex_panel::select(app, true),
            (KeyCode::PageUp, _) => {
                app.view.codex_panel.scroll = app.view.codex_panel.scroll.saturating_sub(5)
            }
            (KeyCode::PageDown, _) => {
                app.view.codex_panel.scroll = app.view.codex_panel.scroll.saturating_add(5)
            }
            (KeyCode::Enter, _) => super::codex_panel::details(app, false),
            (KeyCode::Char('l'), KeyModifiers::NONE) => {
                if app.view.codex_panel.detail {
                    super::codex_panel::details(app, true);
                } else {
                    super::codex_panel::refresh(app, true);
                }
            }
            (KeyCode::Char('r'), KeyModifiers::NONE) => {
                app.session.clear_codex_panel_error();
                if app.view.codex_panel.detail {
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
    let mut rows = PanelRows::default();
    let lines = if size.width == 0 || size.height < 3 {
        Vec::new()
    } else {
        super::codex_panel_text::body(app)
    };
    rows.rebuild(lines, size.width);
    let selected = selected(app);
    draw_panel(
        buf,
        TaskPanel {
            title: "Codex agents / processes",
            title_style: Style::default().fg(COLOR_PRIMARY),
            selected: Some(&selected),
            body: &rows,
            scroll: app.view.codex_panel.scroll,
            footer: if size.width < 40 {
                "x stop F2 back"
            } else {
                "↑↓ select · Enter details · x stop · r latest · l older · PgDn · F3 mode · F4 questions · F2 back"
            },
            compact_help: Some("↑↓ Enter PgDn"),
        },
        size,
    );
}

fn selected(app: &AppState) -> String {
    let panel = &app.view.codex_panel;
    if !app
        .session
        .codex_panel()
        .contains_target(panel.selected.as_ref())
    {
        return if panel.selected.is_some() {
            "Selected row unavailable; use arrows"
        } else {
            "No selected agent/process"
        }
        .into();
    }
    let (label, status) = match panel.selected.as_ref().expect("selected row") {
        PanelId::Agent(id) => {
            let agent = app
                .session
                .codex_panel()
                .agents()
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
    carlos_core::session::text::safe_label(&format!("> {label}: {status}"))
}

fn toggle_panel(app: &mut AppState) {
    app.view.codex_panel.show = !app.view.codex_panel.show;
    if app.view.codex_panel.show {
        if app.view.codex_panel.selected.is_none() {
            app.view.codex_panel.selected = app.session.codex_panel().ids().first().cloned();
        }
        super::codex_panel::refresh(app, false);
    }
}
