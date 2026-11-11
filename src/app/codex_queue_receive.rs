//! Apply native queue replies only to the current thread, epoch and exact request token.

use std::time::Duration;

use serde_json::Value;

use super::notifications::ServerRequestAction;
use super::state::AppState;

/// Invalidate snapshots when any client changes this native queue.
pub(super) fn changed(app: &mut AppState, params: &Value) {
    if params["threadId"] == app.thread_id {
        app.codex.queue.invalidate();
        app.codex.queue.error = None;
    }
}

/// Ignore old-thread replies and retain all native fields in a complete queue snapshot.
pub(super) fn reply(app: &mut AppState, params: &Value) -> Option<ServerRequestAction> {
    if params["requestParams"]["threadId"] != app.thread_id
        || params["requestContext"]["epoch"].as_u64() != Some(app.codex.epoch)
    {
        return None;
    }
    let token = params["requestContext"]["token"].as_u64()?;
    if params["requestMethod"] == "thread/queue/list" {
        return page(app, token, &params["response"]);
    }
    mutation(app, token, params);
    None
}

fn page(app: &mut AppState, token: u64, response: &Value) -> Option<ServerRequestAction> {
    let read = app.codex.queue.read.as_ref()?;
    if token != read.token {
        return None;
    }
    if read.revision != app.codex.queue.revision {
        app.codex.queue.read = None;
        return None;
    }
    if let Some(error) = response.get("error") {
        read_failed(app, &error_message(error));
        return None;
    }
    let result = &response["result"];
    if let Err(error) = append_page(app, result) {
        read_failed(app, &error);
        return None;
    }
    let cursor = result.get("nextCursor").cloned().unwrap_or(Value::Null);
    if !cursor.is_null() {
        return Some(ServerRequestAction::CodexQueuePage { token, cursor });
    }
    let read = app.codex.queue.read.take().expect("queue read");
    app.codex.queue.entries = read.entries;
    app.codex.queue.known = true;
    app.codex.queue.error = None;
    if app.codex.queue.selected.is_none() {
        app.codex.queue.move_selection(0);
    }
    None
}

fn append_page(app: &mut AppState, result: &Value) -> Result<(), String> {
    let data = result["data"].as_array().ok_or("Malformed queue page")?;
    let read = app.codex.queue.read.as_mut().expect("queue read");
    read.bytes = read.bytes.saturating_add(result.to_string().len());
    if read.bytes > 16 * 1024 * 1024
        || read.entries.len() + data.len() > 4096
        || read.started.elapsed() > Duration::from_secs(20)
    {
        return Err("Queue snapshot exceeds 16 MiB, 4096 entries or 20 seconds".into());
    }
    for entry in data {
        let id = entry["id"]
            .as_str()
            .filter(|id| !id.is_empty() && id.len() <= 4096)
            .ok_or("Queue entry has no valid native ID")?;
        if !entry["input"].is_array() || !entry["clientUserMessageId"].is_string() {
            return Err("Queue entry has no native input or client message ID".into());
        }
        if read.entries.iter().any(|previous| previous["id"] == id) {
            return Err("Queue changed across pages; refresh before changing it".into());
        }
        read.entries.push(entry.clone());
    }
    if let Some(cursor) = result.get("nextCursor").filter(|cursor| !cursor.is_null()) {
        let cursor = cursor
            .as_str()
            .filter(|cursor| !cursor.is_empty() && cursor.len() <= 4096)
            .ok_or("Invalid queue cursor")?;
        if !read.cursors.insert(cursor.into()) {
            return Err("Repeated queue cursor".into());
        }
    }
    Ok(())
}

fn mutation(app: &mut AppState, token: u64, params: &Value) {
    let Some(operation) = &app.codex.queue.operation else {
        return;
    };
    if operation.token != token
        || params["requestMethod"] != operation.method
        || params["requestParams"] != operation.params
    {
        return;
    }
    let operation = app.codex.queue.operation.take().expect("queue mutation");
    let result = &params["response"]["result"];
    let error = params["response"]
        .get("error")
        .map(error_message)
        .or_else(|| {
            let entry = &result["queuedSubmission"];
            match operation.method.as_str() {
                "thread/queue/add"
                    if entry["clientUserMessageId"] != operation.params["clientUserMessageId"]
                        || !entry["id"].is_string() =>
                {
                    Some("Queue acknowledgement has the wrong message identity".into())
                }
                "thread/queue/update" if entry["id"] != operation.params["queuedSubmissionId"] => {
                    Some("Queue edit acknowledgement has the wrong entry ID".into())
                }
                "thread/queue/delete" if result["deleted"] != true => {
                    Some("The queued message no longer exists".into())
                }
                _ => None,
            }
        });
    if let Some(error) = error {
        recover(app, operation.submitted);
        super::codex_queue::fail(app, &error);
    } else {
        app.codex.queue.notice = if operation.method == "thread/queue/add" {
            format!(
                "Codex accepted queued message {}",
                result["queuedSubmission"]["id"]
                    .as_str()
                    .unwrap_or("unknown")
            )
        } else if operation.method == "thread/queue/start" {
            "Start accepted; waiting for native turn events".into()
        } else {
            "Codex acknowledged the queue change".into()
        };
        if operation.method == "thread/queue/update" {
            app.codex.queue.edit = None;
        }
        app.codex.queue.error = None;
    }
    app.codex.queue.invalidate();
}

/// Restore an unsent input without replacing text the user typed while awaiting a reply.
pub(super) fn recover(app: &mut AppState, text: Option<String>) {
    if let Some(text) = text {
        if app.input_is_empty() {
            app.set_input_text(&text);
        } else {
            app.codex.queue.recovery = Some(text);
        }
    }
}

fn read_failed(app: &mut AppState, error: &str) {
    app.codex.queue.read = None;
    app.codex.queue.known = false;
    app.codex.queue.error = Some(super::claude_question_state::safe_text(error));
}

fn error_message(error: &Value) -> String {
    super::claude_question_state::safe_text(
        error["message"]
            .as_str()
            .unwrap_or("Codex queue request failed"),
    )
}
