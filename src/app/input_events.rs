//! Terminal event dispatch: keyboard event routing and resize handling.
//! Re-evaluate if a new overlay mode or input subsystem is added.

#[path = "input_events_keys.rs"]
mod input_events_keys;

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use crossterm::event::{Event, KeyCode, KeyModifiers};

use super::mouse_events::{handle_mouse_event, handle_paste_event};
use super::state::AppState;
use super::transcript_render::transcript_content_width;
use super::TerminalSize;
use crate::backend::BackendClient;

use self::input_events_keys::handle_key_event;

// --- Types ---

pub(super) enum TerminalEventResult {
    Quit,
    Continue { needs_draw: bool },
}

// --- Event tracing ---

pub(super) fn trace_terminal_event(ev: &Event) {
    static TRACE_FILE: OnceLock<Option<Mutex<std::fs::File>>> = OnceLock::new();
    let trace = TRACE_FILE.get_or_init(|| {
        let path = std::env::var_os("CARLOS_EVENT_TRACE")?;
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .ok()
            .map(Mutex::new)
    });
    let Some(file_mutex) = trace else { return };
    let Ok(mut file) = file_mutex.lock() else {
        return;
    };
    let _ = match ev {
        Event::Key(k) => writeln!(
            &mut *file,
            "key code={:?} mods={:?} kind={:?}",
            k.code, k.modifiers, k.kind
        ),
        Event::Mouse(m) => writeln!(
            &mut *file,
            "mouse kind={:?} col={} row={} mods={:?}",
            m.kind, m.column, m.row, m.modifiers
        ),
        Event::Paste(p) => writeln!(&mut *file, "paste bytes={} {:?}", p.len(), p),
        Event::Resize(w, h) => writeln!(&mut *file, "resize {} {}", w, h),
        _ => writeln!(&mut *file, "event {:?}", ev),
    };
}

// --- Mobile mouse detection ---

pub(super) fn is_mobile_mouse_key_candidate(
    app: &AppState,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> bool {
    if app.viewport.mobile_plain_pending_coords || app.viewport.mobile_plain_suppress_coords {
        return matches!(code, KeyCode::Char(ch) if ch.is_ascii_digit() || ch == ';');
    }
    if !app.viewport.mobile_mouse_buffer.is_empty() {
        return matches!(code, KeyCode::Char(_));
    }
    let KeyCode::Char(ch) = code else {
        return false;
    };
    ch == '<'
        || (modifiers.contains(KeyModifiers::ALT)
            && matches!(ch, '[' | '<' | ';' | 'M' | 'm' | '0'..='9'))
}

// --- Transcript layout ---

fn hidden_user_message_idx(app: &AppState) -> Option<usize> {
    if app.rewind_mode() {
        app.rewind_selected_message_idx()
    } else {
        None
    }
}

pub(super) fn ensure_transcript_layout(app: &mut AppState, size: TerminalSize) {
    let render_started = Instant::now();
    app.ensure_rendered_lines(transcript_content_width(size), hidden_user_message_idx(app));
    if let Some(perf) = app.perf.as_mut() {
        perf.transcript_render.push(render_started.elapsed());
    }
}

// --- Event dispatch ---

pub(super) fn handle_terminal_event(
    client: &dyn BackendClient,
    app: &mut AppState,
    ev: Event,
    size: TerminalSize,
) -> TerminalEventResult {
    trace_terminal_event(&ev);
    match ev {
        Event::Key(k) => handle_key_event(client, app, k, size),
        Event::Mouse(m) => handle_mouse_event(app, m, size),
        Event::Paste(pasted) => handle_paste_event(app, pasted),
        Event::Resize(_, _) => {
            if let Some(perf) = app.perf.as_mut() {
                perf.resize_events = perf.resize_events.saturating_add(1);
            }
            TerminalEventResult::Continue { needs_draw: true }
        }
        _ => TerminalEventResult::Continue { needs_draw: false },
    }
}
