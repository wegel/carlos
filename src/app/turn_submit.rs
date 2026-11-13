//! Pass accepted text and plain local choices to the session prompt commands.

use super::state::AppState;
use crate::backend::{BackendClient, PromptOrigin};
use crate::session::entry_id::EntryId;
use crate::session::prompt_commands::{PromptContext, PromptOptions, TurnUpdate};

pub(super) use super::turn_approval::respond_to_pending_approval;
pub(super) use super::turn_interrupt::interrupt_active_turn;

#[cfg(test)]
#[path = "turn_submit_history.rs"]
mod history;

/// Submit an ordinary human prompt through the session owner.
pub(super) fn submit_turn_text(client: &dyn BackendClient, app: &mut AppState, text: String) {
    submit_turn_text_with_history(client, app, text, true);
}

/// Submit an edited prompt by its stable session identity.
pub(super) fn submit_rewind_turn_text(
    client: &dyn BackendClient,
    app: &mut AppState,
    selected: EntryId,
    text: String,
) {
    let context = context(&app.view);
    let update = app.session.submit_rewind_text(
        client,
        selected,
        text,
        &context,
        app.view.codex_queue.recovery.is_some(),
    );
    apply(app, update);
}

/// Preserve the caller's explicit history-recording choice.
pub(super) fn submit_turn_text_with_history(
    client: &dyn BackendClient,
    app: &mut AppState,
    text: String,
    record_input_history: bool,
) {
    submit(
        client,
        app,
        text,
        record_input_history,
        false,
        PromptOrigin::Human,
    );
}

/// Dispatch one accepted queue record with its original origin.
pub(super) fn submit_dequeued_turn_text(
    client: &dyn BackendClient,
    app: &mut AppState,
    text: String,
    record_input_history: bool,
    origin: PromptOrigin,
) {
    submit(client, app, text, record_input_history, true, origin);
}

fn submit(
    client: &dyn BackendClient,
    app: &mut AppState,
    text: String,
    record_history: bool,
    force_start: bool,
    origin: PromptOrigin,
) {
    let options = PromptOptions {
        record_history,
        force_start,
        origin,
    };
    let context = context(&app.view);
    let update = app
        .session
        .submit_turn_text(client, text, options, &context);
    apply(app, update);
}

fn context(view: &super::view_state::ViewState) -> PromptContext<'_> {
    PromptContext {
        draft: crate::session::claude::receive_update::ClaudeDraftContext {
            composer: view.input.lines(),
            confirmation: view.confirm_allow,
        },
        claude_queue: view.claude_queue,
        edit_failed: view.claude_edit.failed,
    }
}

/// Apply only the local reports returned by an accepted prompt command.
pub(super) fn apply(app: &mut AppState, update: TurnUpdate) {
    let _ = app.apply_claude_update(update.changes);
    if let Some(hint) = update.hint {
        app.set_hint(hint);
    }
    if update.clear_selection {
        app.view.viewport.selection = None;
    }
}
