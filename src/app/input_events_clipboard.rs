//! Copy a selected range or the last assistant answer from the keyboard.

use crate::app::input_events::TerminalEventResult;
use crate::app::render_input::last_assistant_message;
use crate::app::state::AppState;
use crate::clipboard::{clipboard_backend_label, try_copy_clipboard};

pub(super) fn handle_ctrl_y_copy(app: &mut AppState) -> TerminalEventResult {
    if let Some(sel) = app.view.viewport.selection {
        let copied = app.selected_text(sel);
        if !copied.is_empty() {
            let _ = try_copy_clipboard(&copied);
        }
    } else if let Some(text) = last_assistant_message(&app.session.messages()) {
        let backend = try_copy_clipboard(text);
        app.set_hint(format!(
            "copied last assistant message ({})",
            clipboard_backend_label(backend)
        ));
    }
    TerminalEventResult::Continue { needs_draw: true }
}
