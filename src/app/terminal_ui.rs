//! Terminal setup, thread picker UI, and raw-mode lifecycle helpers.

#[path = "terminal_ui_picker_events.rs"]
mod terminal_ui_picker_events;

use std::cmp::Reverse;
use std::env;
use std::io;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::style::Print;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use super::notifications::is_key_press_like;
use super::picker_render::{compute_picker_layout, draw_picker};
use super::{TerminalSize, ThreadSummary};

use self::terminal_ui_picker_events::{handle_picker_key, handle_picker_mouse};

// --- Terminal Policy ---

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

// --- Terminal Control ---

const MOUSE_CAPTURE_ENABLE_SEQ: &str = "\x1b[?1000h\x1b[?1002h\x1b[?1003h\x1b[?1006h\x1b[?1015h";
const MOUSE_CAPTURE_DISABLE_SEQ: &str = "\x1b[?1015l\x1b[?1006l\x1b[?1003l\x1b[?1002l\x1b[?1000l";

pub(crate) fn sort_threads_for_picker(threads: &[ThreadSummary]) -> Vec<ThreadSummary> {
    let mut sorted = threads.to_vec();
    sorted.sort_by_key(|t| (Reverse(t.updated_at), Reverse(t.created_at), t.id.clone()));
    sorted
}

// --- Terminal Lifecycle ---

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

// --- Picker Types ---

/// Outcome of processing a single event in the picker loop.
enum PickerAction {
    Continue,
    Exit,
    Select(String),
}

pub(super) struct PickerState {
    selected: usize,
    top: usize,
    last_size: TerminalSize,
    confirm_delete: bool,
    status: Option<String>,
}

// --- Thread Picker ---

pub(crate) fn pick_thread<F>(
    threads: &[ThreadSummary],
    allow_delete: bool,
    mut on_delete: F,
) -> Result<Option<String>>
where
    F: FnMut(&ThreadSummary) -> Result<()>,
{
    let mut threads = sort_threads_for_picker(threads);
    if threads.is_empty() {
        return Ok(None);
    }

    with_terminal(|terminal| {
        let mut st = PickerState {
            selected: 0,
            top: 0,
            last_size: TerminalSize {
                width: 0,
                height: 0,
            },
            confirm_delete: false,
            status: None,
        };

        loop {
            draw_picker_frame(terminal, &threads, &mut st, allow_delete)?;

            if !event::poll(Duration::from_millis(15))? {
                continue;
            }

            let ev = event::read()?;
            let action = match ev {
                Event::Key(k) if is_key_press_like(k.kind) => {
                    handle_picker_key(&mut st, &mut threads, allow_delete, &mut on_delete, k)?
                }
                Event::Mouse(m) => handle_picker_mouse(&mut st, &threads, terminal, m)?,
                _ => PickerAction::Continue,
            };

            match action {
                PickerAction::Continue => {}
                PickerAction::Exit => return Ok(None),
                PickerAction::Select(id) => return Ok(Some(id)),
            }
        }
    })
}

// --- Picker Rendering ---

fn draw_picker_frame(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    threads: &[ThreadSummary],
    st: &mut PickerState,
    allow_delete: bool,
) -> Result<()> {
    terminal.draw(|frame| {
        let area = frame.area();
        let size = TerminalSize {
            width: area.width as usize,
            height: area.height as usize,
        };

        if size.width != st.last_size.width || size.height != st.last_size.height {
            st.last_size = size;
        }

        let layout = compute_picker_layout(size);
        let list_height = layout.list_h.saturating_sub(1).max(1);
        if st.selected < st.top {
            st.top = st.selected;
        }
        if st.selected >= st.top + list_height {
            st.top = st.selected + 1 - list_height;
        }

        let delete_target = st
            .confirm_delete
            .then(|| threads.get(st.selected))
            .flatten();
        draw_picker(
            frame,
            threads,
            st.selected,
            st.top,
            allow_delete,
            delete_target,
            st.status.as_deref(),
        );
    })?;
    Ok(())
}
