use std::cmp::Reverse;
use std::env;
use std::io;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyCode, KeyboardEnhancementFlags, MouseButton, MouseEventKind,
    PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::style::Print;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use super::notifications::{is_ctrl_char, is_key_press_like};
use super::picker_render::{compute_picker_layout, draw_picker};
use super::{TerminalSize, ThreadSummary};
use crate::protocol::{extract_result_object, params_thread_archive, AppServerClient};

fn env_flag(name: &str) -> Option<bool> {
    let raw = env::var(name).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Some(true);
    }
    Some(!matches!(
        trimmed.to_ascii_lowercase().as_str(),
        "0" | "false" | "no" | "off"
    ))
}

fn is_ssh_session() -> bool {
    env::var_os("SSH_TTY").is_some()
        || env::var_os("SSH_CONNECTION").is_some()
        || env::var_os("SSH_CLIENT").is_some()
}

fn should_enable_keyboard_enhancement() -> bool {
    if let Some(force) = env_flag("CARLOS_KEYBOARD_ENHANCEMENT") {
        return force;
    }
    true
}

fn should_enable_alternate_scroll() -> bool {
    if let Some(force) = env_flag("CARLOS_ALTERNATE_SCROLL") {
        return force;
    }
    is_ssh_session()
}

const MOUSE_CAPTURE_ENABLE_SEQ: &str = "\x1b[?1000h\x1b[?1002h\x1b[?1003h\x1b[?1006h\x1b[?1015h";
const MOUSE_CAPTURE_DISABLE_SEQ: &str = "\x1b[?1015l\x1b[?1006l\x1b[?1003l\x1b[?1002l\x1b[?1000l";

pub(super) fn sort_threads_for_picker(threads: &[ThreadSummary]) -> Vec<ThreadSummary> {
    let mut sorted = threads.to_vec();
    sorted.sort_by_key(|t| (Reverse(t.updated_at), Reverse(t.created_at), t.id.clone()));
    sorted
}

pub(super) fn with_terminal<T>(
    f: impl FnOnce(&mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<T>,
) -> Result<T> {
    enable_raw_mode().context("failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableBracketedPaste
    )
    .context("failed to enter alt screen")?;
    // Force explicit xterm mouse modes for clients that don't fully honor EnableMouseCapture.
    let _ = execute!(stdout, Print(MOUSE_CAPTURE_ENABLE_SEQ));

    let alternate_scroll_enabled = if should_enable_alternate_scroll() {
        execute!(stdout, Print("\x1b[?1007h")).is_ok()
    } else {
        false
    };

    // Mobile SSH clients can mis-handle kitty keyboard protocol; default it off over SSH.
    let keyboard_enhancement_enabled = if should_enable_keyboard_enhancement() {
        execute!(
            stdout,
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
                    | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
            )
        )
        .is_ok()
    } else {
        false
    };

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("failed to create terminal")?;

    let result = f(&mut terminal);

    let _ = disable_raw_mode();
    if keyboard_enhancement_enabled {
        let _ = execute!(terminal.backend_mut(), PopKeyboardEnhancementFlags);
    }
    if alternate_scroll_enabled {
        let _ = execute!(terminal.backend_mut(), Print("\x1b[?1007l"));
    }
    let _ = execute!(terminal.backend_mut(), Print(MOUSE_CAPTURE_DISABLE_SEQ));
    let _ = execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableBracketedPaste,
        DisableMouseCapture
    );
    let _ = terminal.show_cursor();

    result
}

pub(super) fn pick_thread(
    client: &AppServerClient,
    threads: &[ThreadSummary],
) -> Result<Option<String>> {
    let mut threads = sort_threads_for_picker(threads);
    if threads.is_empty() {
        return Ok(None);
    }

    with_terminal(|terminal| {
        let mut selected = 0usize;
        let mut top = 0usize;
        let mut last_size = TerminalSize {
            width: 0,
            height: 0,
        };
        let mut confirm_delete = false;
        let mut status: Option<String> = None;

        loop {
            terminal.draw(|frame| {
                let area = frame.area();
                let size = TerminalSize {
                    width: area.width as usize,
                    height: area.height as usize,
                };

                if size.width != last_size.width || size.height != last_size.height {
                    last_size = size;
                }

                let layout = compute_picker_layout(size);
                let list_height = layout.list_h.saturating_sub(1).max(1);
                if selected < top {
                    top = selected;
                }
                if selected >= top + list_height {
                    top = selected + 1 - list_height;
                }

                let delete_target = confirm_delete.then(|| threads.get(selected)).flatten();
                draw_picker(
                    frame,
                    &threads,
                    selected,
                    top,
                    delete_target,
                    status.as_deref(),
                );
            })?;

            if !event::poll(Duration::from_millis(15))? {
                continue;
            }

            let ev = event::read()?;
            match ev {
                Event::Key(k) if is_key_press_like(k.kind) => {
                    if confirm_delete {
                        match (k.code, k.modifiers) {
                            (code, mods) if is_ctrl_char(code, mods, 'c') => return Ok(None),
                            (KeyCode::Esc, _) | (KeyCode::Char('n'), _) => {
                                confirm_delete = false;
                                status = None;
                            }
                            (KeyCode::Enter, _) | (KeyCode::Char('y'), _) => {
                                let Some(thread) = threads.get(selected).cloned() else {
                                    confirm_delete = false;
                                    status = None;
                                    continue;
                                };
                                match client.call(
                                    "thread/archive",
                                    params_thread_archive(&thread.id),
                                    Duration::from_secs(20),
                                ) {
                                    Ok(resp) => {
                                        if let Err(err) = extract_result_object(&resp) {
                                            status = Some(format!("archive failed: {err}"));
                                        } else {
                                            threads.remove(selected);
                                            confirm_delete = false;
                                            status = Some(format!("Archived {}", thread.id));
                                            if threads.is_empty() {
                                                return Ok(None);
                                            }
                                            selected =
                                                selected.min(threads.len().saturating_sub(1));
                                        }
                                    }
                                    Err(err) => {
                                        status = Some(format!("archive failed: {err}"));
                                    }
                                }
                            }
                            _ => {}
                        }
                        continue;
                    }

                    match (k.code, k.modifiers) {
                        (code, mods) if is_ctrl_char(code, mods, 'c') => return Ok(None),
                        (KeyCode::Esc, _) => return Ok(None),
                        (KeyCode::Up, _) => {
                            selected = selected.saturating_sub(1);
                            status = None;
                        }
                        (KeyCode::Down, _) => {
                            if selected + 1 < threads.len() {
                                selected += 1;
                                status = None;
                            }
                        }
                        (KeyCode::PageUp, _) => {
                            selected = selected.saturating_sub(10);
                            status = None;
                        }
                        (KeyCode::PageDown, _) => {
                            selected = (selected + 10).min(threads.len().saturating_sub(1));
                            status = None;
                        }
                        (KeyCode::Home, _) | (KeyCode::Char('g'), _) => {
                            selected = 0;
                            status = None;
                        }
                        (KeyCode::End, _) | (KeyCode::Char('G'), _) => {
                            selected = threads.len().saturating_sub(1);
                            status = None;
                        }
                        (KeyCode::Char('j'), _) => {
                            if selected + 1 < threads.len() {
                                selected += 1;
                                status = None;
                            }
                        }
                        (KeyCode::Char('k'), _) => {
                            selected = selected.saturating_sub(1);
                            status = None;
                        }
                        (KeyCode::Char('d'), _) => {
                            confirm_delete = true;
                            status = None;
                        }
                        (KeyCode::Enter, _) => return Ok(Some(threads[selected].id.clone())),
                        _ => {}
                    }
                }
                Event::Mouse(m) => match m.kind {
                    MouseEventKind::ScrollUp => {
                        if !confirm_delete {
                            selected = selected.saturating_sub(1);
                            status = None;
                        }
                    }
                    MouseEventKind::ScrollDown => {
                        if !confirm_delete && selected + 1 < threads.len() {
                            selected += 1;
                            status = None;
                        }
                    }
                    MouseEventKind::Down(MouseButton::Left) => {
                        if confirm_delete {
                            confirm_delete = false;
                            status = None;
                            continue;
                        }
                        let size = terminal.size()?;
                        let layout = compute_picker_layout(TerminalSize {
                            width: size.width as usize,
                            height: size.height as usize,
                        });
                        let row0 = m.row as usize;
                        let data_y = layout.list_y + 1;
                        if row0 >= data_y && row0 < data_y + layout.list_h.saturating_sub(1) {
                            let idx = top + (row0 - data_y);
                            if idx < threads.len() {
                                selected = idx;
                                return Ok(Some(threads[selected].id.clone()));
                            }
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    })
}
