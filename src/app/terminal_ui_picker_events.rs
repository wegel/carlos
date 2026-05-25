// --- Picker Events ---
use std::io;

use anyhow::Result;
use crossterm::event::{KeyCode, MouseButton, MouseEventKind};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use super::{PickerAction, PickerState};
use crate::app::models::ThreadSummary;
use crate::app::notifications::is_ctrl_char;
use crate::app::picker_render::compute_picker_layout;
use crate::app::TerminalSize;

pub(super) fn handle_picker_key<F>(
    st: &mut PickerState,
    threads: &mut Vec<ThreadSummary>,
    allow_delete: bool,
    on_delete: &mut F,
    k: crossterm::event::KeyEvent,
) -> Result<PickerAction>
where
    F: FnMut(&ThreadSummary) -> Result<()>,
{
    if st.confirm_delete {
        return handle_delete_confirm_key(st, threads, on_delete, k);
    }
    handle_normal_key(st, threads, allow_delete, k)
}

fn handle_delete_confirm_key<F>(
    st: &mut PickerState,
    threads: &mut Vec<ThreadSummary>,
    on_delete: &mut F,
    k: crossterm::event::KeyEvent,
) -> Result<PickerAction>
where
    F: FnMut(&ThreadSummary) -> Result<()>,
{
    match (k.code, k.modifiers) {
        (code, mods) if is_ctrl_char(code, mods, 'c') => return Ok(PickerAction::Exit),
        (KeyCode::Esc, _) | (KeyCode::Char('n'), _) => {
            st.confirm_delete = false;
            st.status = None;
        }
        (KeyCode::Enter, _) | (KeyCode::Char('y'), _) => {
            return execute_delete(st, threads, on_delete);
        }
        _ => {}
    }
    Ok(PickerAction::Continue)
}

fn execute_delete<F>(
    st: &mut PickerState,
    threads: &mut Vec<ThreadSummary>,
    on_delete: &mut F,
) -> Result<PickerAction>
where
    F: FnMut(&ThreadSummary) -> Result<()>,
{
    let Some(thread) = threads.get(st.selected).cloned() else {
        st.confirm_delete = false;
        st.status = None;
        return Ok(PickerAction::Continue);
    };
    match on_delete(&thread) {
        Ok(()) => {
            threads.remove(st.selected);
            st.confirm_delete = false;
            st.status = Some(format!("Archived {}", thread.id));
            if threads.is_empty() {
                return Ok(PickerAction::Exit);
            }
            st.selected = st.selected.min(threads.len().saturating_sub(1));
        }
        Err(err) => {
            st.status = Some(format!("archive failed: {err}"));
        }
    }
    Ok(PickerAction::Continue)
}

fn handle_normal_key(
    st: &mut PickerState,
    threads: &[ThreadSummary],
    allow_delete: bool,
    k: crossterm::event::KeyEvent,
) -> Result<PickerAction> {
    match (k.code, k.modifiers) {
        (code, mods) if is_ctrl_char(code, mods, 'c') => return Ok(PickerAction::Exit),
        (KeyCode::Esc, _) => return Ok(PickerAction::Exit),
        (KeyCode::Up | KeyCode::Char('k'), _) => {
            st.selected = st.selected.saturating_sub(1);
            st.status = None;
        }
        (KeyCode::Down | KeyCode::Char('j'), _) => {
            if st.selected + 1 < threads.len() {
                st.selected += 1;
                st.status = None;
            }
        }
        (KeyCode::PageUp, _) => {
            st.selected = st.selected.saturating_sub(10);
            st.status = None;
        }
        (KeyCode::PageDown, _) => {
            st.selected = (st.selected + 10).min(threads.len().saturating_sub(1));
            st.status = None;
        }
        (KeyCode::Home | KeyCode::Char('g'), _) => {
            st.selected = 0;
            st.status = None;
        }
        (KeyCode::End | KeyCode::Char('G'), _) => {
            st.selected = threads.len().saturating_sub(1);
            st.status = None;
        }
        (KeyCode::Char('d'), _) if allow_delete => {
            st.confirm_delete = true;
            st.status = None;
        }
        (KeyCode::Enter, _) => {
            return Ok(PickerAction::Select(threads[st.selected].id.clone()));
        }
        _ => {}
    }
    Ok(PickerAction::Continue)
}

pub(super) fn handle_picker_mouse(
    st: &mut PickerState,
    threads: &[ThreadSummary],
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    m: crossterm::event::MouseEvent,
) -> Result<PickerAction> {
    match m.kind {
        MouseEventKind::ScrollUp if !st.confirm_delete => {
            st.selected = st.selected.saturating_sub(1);
            st.status = None;
        }
        MouseEventKind::ScrollDown if !st.confirm_delete && st.selected + 1 < threads.len() => {
            st.selected += 1;
            st.status = None;
        }
        MouseEventKind::Down(MouseButton::Left) => {
            return handle_mouse_click(st, threads, terminal, m);
        }
        _ => {}
    }
    Ok(PickerAction::Continue)
}

fn handle_mouse_click(
    st: &mut PickerState,
    threads: &[ThreadSummary],
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    m: crossterm::event::MouseEvent,
) -> Result<PickerAction> {
    if st.confirm_delete {
        st.confirm_delete = false;
        st.status = None;
        return Ok(PickerAction::Continue);
    }
    let size = terminal.size()?;
    let layout = compute_picker_layout(TerminalSize {
        width: size.width as usize,
        height: size.height as usize,
    });
    let row0 = m.row as usize;
    let data_y = layout.list_y + 1;
    if row0 >= data_y && row0 < data_y + layout.list_h.saturating_sub(1) {
        let idx = st.top + (row0 - data_y);
        if idx < threads.len() {
            st.selected = idx;
            return Ok(PickerAction::Select(threads[st.selected].id.clone()));
        }
    }
    Ok(PickerAction::Continue)
}
