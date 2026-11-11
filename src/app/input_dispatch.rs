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
    if crate::app::codex_reconnect::receive(client, app, line) {
        return;
    }
    if let Some(action) = handle_server_message_line(app, line) {
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
    match action {
        ServerRequestAction::CodexQueuePage { token, cursor } => {
            crate::app::codex_queue::page(client, app, token, cursor);
        }
        ServerRequestAction::ValidateCodexForm {
            id,
            schema,
            answer,
            revision,
        } => {
            if let Err(error) = client.validate_form(id.clone(), schema, answer, revision) {
                crate::app::codex_callbacks::reply_error(app, &id, &error.to_string());
            }
        }
        ServerRequestAction::ReplyNative { id, result } => {
            crate::app::codex_reply::send(client, app, &id, result);
        }
        ServerRequestAction::ClaudeControl { method, params } => {
            if let Err(error) = client.request_native_control(&method, params) {
                crate::app::claude_notifications::fail_session(app);
                app.set_status(format!("Claude session failed: {error}"));
            }
        }
        ServerRequestAction::ApplyClaudeSettings => {
            let (model, effort, _) = app.next_turn_runtime_settings();
            let (model, effort) = crate::app::claude_models::prompt_choices(app, model, effort);
            match client.apply_runtime_settings(model.as_deref(), effort.as_deref()) {
                Ok(true) => {
                    crate::app::claude_context::invalidate(app);
                    app.mark_runtime_settings_applied();
                }
                Ok(false) => {}
                Err(error) => {
                    app.ralph_runtime.stop_after_failure();
                    app.set_status(format!("Could not apply Claude settings: {error}"));
                }
            }
            if let Err(error) = read_context(client, app) {
                app.set_status(format!("Could not read context: {error}"));
            }
        }
        ServerRequestAction::ReplyError {
            request_id,
            code,
            message,
        } => {
            if let Err(err) = client.respond_error(&request_id, code, &message) {
                app.set_status(format!("server request reply failed: {err}"));
            }
        }
        ServerRequestAction::RefreshContext => {
            if let Err(error) = read_context(client, app) {
                app.set_status(format!("Could not read context: {error}"));
            }
        }
    }
}

/// Tag native context reads so late pre-compaction replies cannot restore stale counts.
fn read_context(client: &dyn BackendClient, app: &mut AppState) -> anyhow::Result<()> {
    if client.kind() == crate::backend::BackendKind::Claude {
        crate::app::claude_context::refresh(app);
        Ok(())
    } else {
        client.refresh_context()
    }
}
