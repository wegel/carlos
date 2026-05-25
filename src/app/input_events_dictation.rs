//! Dictation key shortcuts: Ctrl-D toggle, F7 profile cycling, F9 endpoint switching.

use super::TerminalEventResult;
use crate::app::state::AppState;

pub(super) fn handle_f7_dictation_profile(app: &mut AppState) -> TerminalEventResult {
    app.switch_dictation_profile_next();
    TerminalEventResult::Continue { needs_draw: true }
}

pub(super) fn handle_f9_dictation_endpoint(app: &mut AppState) -> TerminalEventResult {
    app.toggle_dictation_endpoint_mode();
    TerminalEventResult::Continue { needs_draw: true }
}

pub(super) fn handle_ctrl_d_dictation(app: &mut AppState) -> TerminalEventResult {
    if app.dictation_recording() {
        app.stop_dictation_recording();
    } else if app.dictation_active() {
        app.restart_dictation_recording();
    } else {
        app.start_dictation_recording();
    }
    TerminalEventResult::Continue { needs_draw: true }
}

pub(super) fn handle_enter_stop_dictation(app: &mut AppState) -> TerminalEventResult {
    app.stop_dictation_recording();
    TerminalEventResult::Continue { needs_draw: true }
}
