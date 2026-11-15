//! Trace local terminal events while masking the currently focused secret editor.

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{Mutex, OnceLock};

use crossterm::event::Event;

pub(super) fn trace_terminal_event(ev: &Event, secret: bool) {
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
    if secret && matches!(ev, Event::Key(_) | Event::Paste(_)) {
        let _ = writeln!(&mut *file, "secret input (redacted)");
        return;
    }
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
