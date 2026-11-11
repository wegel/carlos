//! Send native queue controls without locally dispatching or echoing queued prompts.

use std::collections::HashSet;
use std::time::Instant;

use ratatui_textarea::TextArea;
use serde_json::{json, Value};
use uuid::Uuid;

use super::codex_queue_state::{editable_text, QueueEdit, QueueOperation, QueueRead};
use super::state::AppState;
use crate::backend::BackendClient;

/// Keep uncertain queue writes and recoverable drafts in their original thread before a fork.
pub(super) fn reject_switch(app: &mut AppState) -> bool {
    if app.codex.queue.operation.is_none() && app.codex.queue.recovery.is_none() {
        return false;
    }
    app.append_message(super::Role::System, "Finish or recover the Codex queue request before editing earlier history. This edited prompt is preserved.");
    true
}

/// Consume local queue commands before normal prompt/history submission.
pub(super) fn command(client: &dyn BackendClient, app: &mut AppState, text: &str) -> bool {
    if !app.codex.enabled {
        return false;
    }
    if text.trim() == "/queue" {
        app.clear_input();
        open(app);
    } else if app.codex.queue.mode {
        submit(client, app, text);
    } else {
        return false;
    }
    true
}

/// Open the native queue without using or replacing the main composer draft.
pub(super) fn open(app: &mut AppState) {
    app.codex.queue.show = true;
    if app.codex.queue.selected.is_none() {
        app.codex.queue.move_selection(0);
    }
    app.codex.queue.error = None;
    app.codex.queue.invalidate();
}

/// Start a bounded snapshot when notifications or a thread switch invalidate it.
pub(super) fn tick(client: &dyn BackendClient, app: &mut AppState) -> bool {
    if !app.codex.enabled
        || app.codex.closed
        || app.codex.queue.known
        || app.codex.queue.read.is_some()
        || app.codex.queue.error.is_some()
    {
        return false;
    }
    app.codex.serial = app.codex.serial.saturating_add(1);
    let token = app.codex.serial;
    app.codex.queue.read = Some(QueueRead {
        token,
        revision: app.codex.queue.revision,
        started: Instant::now(),
        entries: Vec::new(),
        cursors: HashSet::new(),
        bytes: 0,
    });
    page(client, app, token, Value::Null);
    true
}

/// Read one record per frame so large persisted inputs do not overflow a native reply.
pub(super) fn page(client: &dyn BackendClient, app: &mut AppState, token: u64, cursor: Value) {
    let Some(read) = app
        .codex
        .queue
        .read
        .as_ref()
        .filter(|read| read.token == token)
    else {
        return;
    };
    let remaining = std::time::Duration::from_secs(20).saturating_sub(read.started.elapsed());
    if remaining.is_zero() {
        app.codex.queue.read = None;
        app.codex.queue.error = Some("Queue read exceeded its 20 second deadline".into());
        return;
    }
    let params = json!({"threadId":app.thread_id,"cursor":cursor,"limit":1});
    let mut local = context(app, token);
    local["timeoutMs"] = json!(remaining.as_millis().min(15000) as u64);
    if let Err(error) = client.request_control_with_context("thread/queue/list", params, local) {
        app.codex.queue.read = None;
        fail(app, &format!("Could not read Codex queue: {error}"));
    }
}

/// Queue a human message once; only native consumed user items enter transcript/history.
pub(super) fn submit(client: &dyn BackendClient, app: &mut AppState, text: &str) {
    if app.codex.queue.operation.is_some() || app.codex.queue.recovery.is_some() {
        fail(
            app,
            "Finish or recover the previous queue request before adding another",
        );
        return;
    }
    let params = json!({"threadId":app.thread_id,"clientUserMessageId":Uuid::new_v4().to_string(),
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
    if app.codex.closed || app.codex.queue.operation.is_some() {
        fail(app, "A queue request is still pending, or Codex is closed");
        return false;
    }
    app.codex.serial = app.codex.serial.saturating_add(1);
    let token = app.codex.serial;
    let mut identity = params.clone();
    identity
        .as_object_mut()
        .expect("queue params")
        .remove("input");
    let result = client.request_control_with_context(method, params, context(app, token));
    if let Err(error) = result {
        fail(app, &format!("Could not send queue request: {error}"));
        return false;
    }
    app.codex.queue.operation = Some(QueueOperation {
        token,
        method: method.into(),
        params: identity,
        submitted,
    });
    app.codex.queue.error = None;
    app.codex.queue.notice = "Queue request pending; Codex has not acknowledged it".into();
    app.codex.queue.invalidate();
    true
}

/// Preserve a selected persisted payload unless Carlos can safely edit all of it.
pub(super) fn begin_edit(app: &mut AppState) {
    let Some(entry) = app.codex.queue.selected_entry() else {
        fail(app, "Select an existing queued message first");
        return;
    };
    let Some(text) = editable_text(entry) else {
        fail(app, "Edit unavailable: this message has attachments, spans or unsupported input; start/delete/reorder remain available");
        return;
    };
    app.codex.queue.edit = Some(QueueEdit {
        id: entry["id"].as_str().expect("native queue ID").into(),
        original: entry["input"].clone(),
        editor: TextArea::new(text.split('\n').map(str::to_string).collect()),
    });
    app.codex.queue.error = None;
}

/// Reject stale edits rather than overwriting another client's queued message.
pub(super) fn save_edit(client: &dyn BackendClient, app: &mut AppState) {
    let Some(edit) = &app.codex.queue.edit else {
        return;
    };
    let fresh = app.codex.queue.known
        && app
            .codex
            .queue
            .entries
            .iter()
            .any(|entry| entry["id"] == edit.id && entry["input"] == edit.original);
    if !fresh {
        fail(
            app,
            "Queue changed; this draft is preserved. Refresh and reselect before saving",
        );
        return;
    }
    let text = edit.editor.lines().join("\n");
    let params = json!({"threadId":app.thread_id,"queuedSubmissionId":edit.id,
        "input":[{"type":"text","text":text,"text_elements":[]}]});
    mutate(client, app, "thread/queue/update", params, None);
}

/// Delete/start/reorder only a fresh explicitly selected native ID.
pub(super) fn operate(client: &dyn BackendClient, app: &mut AppState, command: &str) {
    if !app.codex.queue.known || app.codex.queue.selected_entry().is_none() {
        fail(
            app,
            "Select a current queued message after the queue finishes refreshing",
        );
        return;
    }
    let id = app.codex.queue.selected.clone().expect("selected queue ID");
    let (method, params) = if matches!(command, "up" | "down") {
        let mut ids = app
            .codex
            .queue
            .entries
            .iter()
            .map(|entry| entry["id"].as_str().expect("queue ID").to_string())
            .collect::<Vec<_>>();
        let index = ids
            .iter()
            .position(|entry| *entry == id)
            .expect("selected entry");
        let next = if command == "up" {
            index.saturating_sub(1)
        } else {
            (index + 1).min(ids.len() - 1)
        };
        ids.swap(index, next);
        (
            "thread/queue/reorder",
            json!({"threadId":app.thread_id,"queuedSubmissionIds":ids}),
        )
    } else {
        let method = if command == "start" {
            "thread/queue/start"
        } else {
            "thread/queue/delete"
        };
        (
            method,
            json!({"threadId":app.thread_id,"queuedSubmissionId":id}),
        )
    };
    mutate(client, app, method, params, None);
}

/// Keep failures visible in the queue panel while preserving both editors.
pub(super) fn fail(app: &mut AppState, message: &str) {
    app.codex.queue.error = Some(super::claude_question_state::safe_text(message));
    app.codex.queue.show = true;
}

/// Let Carlos automatic work run only after an authoritative empty native queue read.
pub(super) fn allows_automatic(app: &AppState) -> bool {
    !app.codex.enabled
        || (app.codex.queue.known
            && app.codex.queue.entries.is_empty()
            && app.codex.queue.operation.is_none()
            && !app.codex.closed)
}

fn context(app: &AppState, token: u64) -> Value {
    json!({"epoch":app.codex.epoch,"token":token})
}
