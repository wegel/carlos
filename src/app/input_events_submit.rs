//! Route local commands after literal rewind edits and preserve native prompt ownership.

use crate::app::input_events::TerminalEventResult;
use crate::app::session_name::handle_rename_command;
use crate::app::state::AppState;
use crate::app::tool_output_command::handle_tools_command;
use crate::app::turn_submit::{submit_rewind_turn_text, submit_turn_text};
use crate::backend::BackendClient;

/// Submit the user's exact text without intercepting /goal inside a selected rewind edit.
pub(super) fn handle_enter_submit(
    client: &dyn BackendClient,
    app: &mut AppState,
) -> TerminalEventResult {
    if app.claude.enabled && crate::app::claude_edit::waiting(app) {
        return TerminalEventResult::Continue { needs_draw: true };
    }
    if app.input_is_empty() {
        return TerminalEventResult::Continue { needs_draw: true };
    }

    let text = app.input_text();
    if app.codex.enabled && app.codex.closed {
        app.set_status(if app.codex.connection.rebuilding {
            "Codex reconnect is still reading; draft kept"
        } else {
            "Codex connection lost; F5 reconnects. Draft kept"
        });
        return TerminalEventResult::Continue { needs_draw: true };
    }
    if handle_tools_command(app, &text) {
        return TerminalEventResult::Continue { needs_draw: true };
    }
    if app.rewind_mode() {
        if let Some(rewind_history_idx) = app.rewind_selected_history_index() {
            submit_rewind_turn_text(client, app, rewind_history_idx, text);
            return TerminalEventResult::Continue { needs_draw: true };
        }
        if app.claude.enabled {
            crate::app::claude_edit::unavailable(
                app,
                "Choose a saved prompt with Up/Down; F4 selects older prompts with unknown source",
            );
            return TerminalEventResult::Continue { needs_draw: true };
        }
    }

    if handle_rename_command(client, app, &text) {
        return TerminalEventResult::Continue { needs_draw: true };
    }
    if crate::app::codex_queue::command(client, app, &text)
        || crate::app::codex_goals::command(app, &text)
    {
        return TerminalEventResult::Continue { needs_draw: true };
    }
    app.clear_rewind_mode_state();
    app.push_input_history(&text);
    app.clear_input();
    app.viewport.selection = None;
    app.mark_user_turn_submitted();
    submit_turn_text(client, app, text);
    TerminalEventResult::Continue { needs_draw: true }
}
