//! Per-event dispatch: route a UI event to terminal/server/dictation handlers.

use std::time::Instant;

use crate::app::input_events::{handle_terminal_event, TerminalEventResult};
use crate::app::notifications::{handle_server_message_line, ServerRequestAction};
use crate::app::{AppState, TerminalSize};
use crate::backend::BackendClient;
use crate::event::UiEvent;

/// Handle a single UI event. Returns `true` when the caller should quit.
pub(super) fn process_event(
    event: UiEvent,
    client: &dyn BackendClient,
    app: &mut AppState,
    size: TerminalSize,
    needs_draw: &mut bool,
) -> bool {
    let event_started = Instant::now();
    let quit = match event {
        UiEvent::ServerLine(line) => {
            handle_server_line(client, app, &line);
            *needs_draw = true;
            false
        }
        #[cfg(feature = "dictation")]
        UiEvent::Dictation(event) => {
            app.handle_dictation_event(event);
            *needs_draw = true;
            false
        }
        UiEvent::Terminal(ev) => match handle_terminal_event(client, app, ev, size) {
            TerminalEventResult::Quit => true,
            TerminalEventResult::Continue {
                needs_draw: event_needs_draw,
            } => {
                if event_needs_draw {
                    *needs_draw = true;
                }
                false
            }
        },
    };
    if let Some(perf) = app.perf.as_mut() {
        perf.event_handle.push(event_started.elapsed());
    }
    quit
}

/// Process a single server JSON line, dispatching any resulting actions.
fn handle_server_line(client: &dyn BackendClient, app: &mut AppState, line: &str) {
    if let Some(perf) = app.perf.as_mut() {
        perf.notifications = perf.notifications.saturating_add(1);
    }
    if let Some(action) = handle_server_message_line(app, line) {
        match action {
            ServerRequestAction::ReplyError {
                request_id,
                code,
                message,
            } => {
                if let Err(err) = client.respond_error(&request_id, code, &message) {
                    app.set_status(format!("server request reply failed: {err}"));
                }
            }
        }
    }
}
