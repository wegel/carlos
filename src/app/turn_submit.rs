//! Pass accepted text and plain local choices to the session prompt commands.

use carlos_core::backend::{BackendClient, PromptOrigin};
use carlos_core::session::claude::receive_update::{ClaudeEffect, ClaudeUpdate};
use carlos_core::session::command_work::{CommandError, PreparedCommand};
use carlos_core::session::entry_id::EntryId;
use carlos_core::session::prompt_commands::{PromptContext, PromptOptions, TurnUpdate};
use carlos_proto::submissions::{RecoveryKind, SubmissionOutcome};

use super::state::AppState;

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
    let update = app.session.prepare_rewind_text(
        client.kind(),
        selected,
        text,
        &context,
        app.view.codex_queue.recovery.is_some(),
    );
    run_prepared(client, app, update);
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
#[cfg(test)]
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
        .prepare_turn_text(client.kind(), text, options, &context);
    run_prepared(client, app, update);
}

fn context(view: &super::view_state::ViewState) -> PromptContext {
    PromptContext {
        operation: Some(view.new_operation()),
        claude_queue: view.claude_queue,
    }
}

/// Apply only the local reports returned by an accepted prompt command.
pub(super) fn apply(app: &mut AppState, update: TurnUpdate) {
    let _ = app.apply_claude_update(update.changes);
    if let Some(hint) = update.hint {
        app.set_hint(hint);
    }
    if let Some(name) = update.name_saved {
        if app.input_text().trim() == format!("/rename {name}") {
            app.clear_input();
        }
    }
    if update.rewound {
        app.clear_rewind_mode_state();
    }
    if let Some(text) = update.submitted {
        if app.input_text() == text {
            app.clear_input();
        }
    }
    if update.clear_selection {
        app.view.viewport.selection = None;
    }
}

/// Dispatch a queued command with the same owner that submitted it.
pub(super) fn submit_queued(
    client: &dyn BackendClient,
    app: &mut AppState,
    queued: carlos_core::session::ralph_runtime_state::QueuedTurnInput,
) {
    let mut context = context(&app.view);
    context.operation = queued.operation;
    let options = PromptOptions {
        record_history: queued.record_input_history,
        force_start: true,
        origin: queued.origin,
    };
    let update = app
        .session
        .prepare_turn_text(client.kind(), queued.text, options, &context);
    run_prepared(client, app, update);
}

/// Execute between session calls; EP019 schedules the identical work on its backend worker.
pub(super) fn run_prepared(
    client: &dyn BackendClient,
    app: &mut AppState,
    mut prepared: Result<PreparedCommand, CommandError>,
) {
    loop {
        let command = match prepared {
            Ok(command) => command,
            Err(error) => {
                let hint = error.to_string();
                if let Some(draft) = error.into_submitted() {
                    let mut update = ClaudeUpdate::default();
                    update
                        .effects
                        .push(ClaudeEffect::Submission(SubmissionOutcome::Recovered {
                            draft,
                            kind: RecoveryKind::Prompt,
                        }));
                    app.apply_claude_update(update);
                }
                app.set_hint(hint);
                return;
            }
        };
        apply(app, command.update);
        let Some(work) = command.work else {
            return;
        };
        let completion = work.execute(client);
        prepared = app.session.finish_backend_work(completion);
    }
}
