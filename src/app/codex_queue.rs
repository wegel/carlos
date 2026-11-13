//! Send native queue controls without locally dispatching or echoing queued prompts.

use ratatui_textarea::TextArea;
use serde_json::{json, Value};
use uuid::Uuid;

use super::codex_queue_state::QueueEdit;
use super::state::AppState;
use carlos_core::backend::BackendClient;
use carlos_core::session::codex::queue::{self, CodexScope, QueueUpdate};
use carlos_core::session::codex::queue_state::editable_text;

/// Keep uncertain queue writes and recoverable drafts in their original thread before a fork.
#[cfg(test)]
pub(super) fn reject_switch(app: &mut AppState) -> bool {
    if !app.session.codex_queue().pending() && app.view.codex_queue.recovery.is_none() {
        return false;
    }
    app.append_message(carlos_core::session::models::Role::System, "Finish or recover the Codex queue request before editing earlier history. This edited prompt is preserved.");
    true
}

/// Consume local queue commands before normal prompt/history submission.
pub(super) fn command(client: &dyn BackendClient, app: &mut AppState, text: &str) -> bool {
    if !app.session.codex_scope().enabled() {
        return false;
    }
    if text.trim() == "/queue" {
        app.clear_input();
        open(app);
    } else if app.view.codex_queue.mode {
        submit(client, app, text);
    } else {
        return false;
    }
    true
}

/// Open the native queue without using or replacing the main composer draft.
pub(super) fn open(app: &mut AppState) {
    app.view.codex_queue.show = true;
    if app.view.codex_queue.selected.is_none() {
        app.view
            .codex_queue
            .move_selection(0, app.session.codex_queue());
    }
    app.view.codex_queue.error = None;
    app.session.clear_codex_queue_error();
    app.session.invalidate_codex_queue();
}

/// Start a bounded snapshot when notifications or a thread switch invalidate it.
pub(super) fn tick(client: &dyn BackendClient, app: &mut AppState) -> bool {
    let scope = scope(app);
    let mut update = QueueUpdate::default();
    let changed = queue::tick(
        client,
        &mut app.session,
        scope,
        &mut update,
        app.view.codex_queue.error.is_some(),
    );
    apply(app, update);
    changed
}

/// Read one record per frame so large persisted inputs do not overflow a native reply.
#[cfg(test)]
pub(super) fn page(client: &dyn BackendClient, app: &mut AppState, token: u64, cursor: Value) {
    let scope = scope(app);
    let mut update = QueueUpdate::default();
    queue::page(client, &mut app.session, scope, &mut update, token, cursor);
    apply(app, update);
}

/// Queue a human message once; only native consumed user items enter transcript/history.
pub(super) fn submit(client: &dyn BackendClient, app: &mut AppState, text: &str) {
    if app.session.codex_queue().pending() || app.view.codex_queue.recovery.is_some() {
        fail(
            app,
            "Finish or recover the previous queue request before adding another",
        );
        return;
    }
    let params = json!({"threadId":app.session.thread_id(),"clientUserMessageId":Uuid::new_v4().to_string(),
        "input":[{"type":"text","text":text,"text_elements":[]}]});
    if mutate(
        client,
        app,
        "thread/queue/add",
        params,
        Some(text.to_string()),
    ) {
        app.clear_input();
    }
}

/// Keep one outstanding mutation with an exact local token and recoverable input.
pub(super) fn mutate(
    client: &dyn BackendClient,
    app: &mut AppState,
    method: &str,
    params: Value,
    submitted: Option<String>,
) -> bool {
    let scope = scope(app);
    let mut update = QueueUpdate::default();
    let sent = queue::mutate(
        client,
        &mut app.session,
        scope,
        &mut update,
        method,
        params,
        submitted,
    );
    apply(app, update);
    sent
}

/// Preserve a selected persisted payload unless Carlos can safely edit all of it.
pub(super) fn begin_edit(app: &mut AppState) {
    let Some(entry) = app
        .view
        .codex_queue
        .selected_entry(app.session.codex_queue())
    else {
        fail(app, "Select an existing queued message first");
        return;
    };
    let Some(text) = editable_text(entry) else {
        fail(app, "Edit unavailable: this message has attachments, spans or unsupported input; start/delete/reorder remain available");
        return;
    };
    app.view.codex_queue.edit = Some(QueueEdit {
        id: entry["id"].as_str().expect("native queue ID").into(),
        original: entry["input"].clone(),
        editor: TextArea::new(text.split('\n').map(str::to_string).collect()),
    });
    app.view.codex_queue.error = None;
    app.session.clear_codex_queue_error();
}

/// Reject stale edits rather than overwriting another client's queued message.
pub(super) fn save_edit(client: &dyn BackendClient, app: &mut AppState) {
    let Some(edit) = &app.view.codex_queue.edit else {
        return;
    };
    let scope = scope(app);
    let mut update = QueueUpdate::default();
    let result = carlos_core::session::codex::queue_edit::save_edit(
        client,
        &mut app.session,
        scope,
        &mut update,
        &edit.id,
        &edit.original,
        edit.editor.lines().join("\n"),
    );
    apply(app, update);
    if let Err(error) = result {
        fail(app, error);
    }
}

/// Supply the selected native identity without calculating backend queue order in the view.
pub(super) fn operate(client: &dyn BackendClient, app: &mut AppState, command: &str) {
    let scope = scope(app);
    let mut update = QueueUpdate::default();
    let result = carlos_core::session::codex::queue_edit::operate(
        client,
        &mut app.session,
        scope,
        &mut update,
        app.view.codex_queue.selected.as_deref(),
        command,
    );
    apply(app, update);
    if let Err(error) = result {
        fail(app, error);
    }
}

/// Keep failures visible in the queue panel while preserving both editors.
pub(super) fn fail(app: &mut AppState, message: &str) {
    app.view.codex_queue.error = Some(carlos_core::session::text::safe_text(message));
    app.view.codex_queue.show = true;
}

/// Let Carlos automatic work run only after an authoritative empty native queue read.
#[cfg(test)]
pub(super) fn allows_automatic(app: &AppState) -> bool {
    queue::allows_automatic(&app.session, scope(app))
}

/// Describe the current native connection for the queue controller.
pub(super) fn scope(app: &AppState) -> CodexScope {
    app.session.codex_scope()
}

/// Apply each returned recovery draft and editor change exactly once.
pub(super) fn apply(app: &mut AppState, update: QueueUpdate) {
    if update.clear_error {
        app.view.codex_queue.error = None;
    }
    if update.show {
        app.view.codex_queue.show = true;
    }
    if update.clear_edit {
        app.view.codex_queue.edit = None;
    }
    if update.select_first && app.view.codex_queue.selected.is_none() {
        app.view
            .codex_queue
            .move_selection(0, app.session.codex_queue());
    }
    super::codex_queue_receive::recover(app, update.recovery);
}

/// Show the latest local failure before the last backend failure.
pub(super) fn error(app: &AppState) -> Option<&str> {
    app.view
        .codex_queue
        .error
        .as_deref()
        .or(app.session.codex_queue().error())
}
