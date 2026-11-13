//! Per-event dispatch: route a UI event to terminal/server/dictation handlers.

use std::time::Instant;

use carlos_core::backend::BackendClient;
use carlos_core::session::actions::ServerRequestAction;

use crate::app::input_events::{handle_terminal_event, TerminalEventResult};
use crate::app::notifications::handle_server_message_line;
use crate::app::{AppState, TerminalSize};
use crate::event::UiEvent;

/// Handle a single UI event. Returns `true` when the caller should quit.
pub(in crate::app) fn process_event(
    event: UiEvent,
    client: &dyn BackendClient,
    app: &mut AppState,
    size: TerminalSize,
    needs_draw: &mut bool,
) -> bool {
    let event_started = Instant::now();
    let quit = match event {
        UiEvent::Summary(event) => {
            app.handle_summary_event(event);
            *needs_draw = true;
            false
        }
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
    if let Some(perf) = app.view.perf.as_mut() {
        perf.event_handle.push(event_started.elapsed());
    }
    quit
}

/// Process a single server JSON line, dispatching any resulting actions.
fn handle_server_line(client: &dyn BackendClient, app: &mut AppState, line: &str) {
    if let Some(perf) = app.view.perf.as_mut() {
        perf.notifications = perf.notifications.saturating_add(1);
    }
    if crate::app::codex_reconnect::receive(client, app, line) {
        return;
    }
    if let Some(lines) = client.handle_event(line) {
        // A native result applies its completions before its context-refresh action, as on main.
        let actions: Vec<_> = lines
            .iter()
            .filter_map(|handled| handle_server_message_line(app, handled))
            .collect();
        for action in actions {
            dispatch_action(client, app, action);
        }
    } else if let Some(action) = handle_server_message_line(app, line) {
        dispatch_action(client, app, action);
    }
    client.acknowledge_event(line.len());
}

/// Run one reply/control after the matching native frame passes its connection fence.
pub(in crate::app) fn dispatch_action(
    client: &dyn BackendClient,
    app: &mut AppState,
    action: ServerRequestAction,
) {
    let context = super::super::backend_updates::context(&app.session, &app.view);
    let update = app.session.run_backend_action(client, action, &context);
    let _ = app.apply_backend_update(update);
}
