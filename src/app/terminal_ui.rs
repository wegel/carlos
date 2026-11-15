//! Terminal setup, thread picker UI, and raw-mode lifecycle helpers.

use std::env;
use std::io;

use anyhow::{Context, Result};
use crossterm::event::{
    DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::style::Print;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

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

// --- Thread Picker ---

// --- Picker Rendering ---

/// Share the old picker movement rules across embedded rows and daemon pages.
pub(in crate::app) fn move_picker_selection(
    code: crossterm::event::KeyCode,
    selected: usize,
    len: usize,
) -> Option<usize> {
    use crossterm::event::KeyCode;
    Some(match code {
        KeyCode::Up | KeyCode::Char('k') => selected.saturating_sub(1),
        KeyCode::Down | KeyCode::Char('j') => (selected + 1).min(len.saturating_sub(1)),
        KeyCode::PageUp => selected.saturating_sub(10),
        KeyCode::PageDown => (selected + 10).min(len.saturating_sub(1)),
        KeyCode::Home | KeyCode::Char('g') => 0,
        KeyCode::End | KeyCode::Char('G') => len.saturating_sub(1),
        _ => return None,
    })
}
