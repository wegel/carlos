//! Apply native queue replies only to the current thread, epoch and exact request token.

use std::time::Duration;

use serde_json::Value;

use super::queue::{CodexScope, QueueUpdate};
use crate::session::state::SessionState;

/// Invalidate snapshots when any client changes this native queue.
pub fn changed(session: &mut SessionState, update: &mut QueueUpdate, params: &Value) {
    if params["threadId"] == *session.thread_id() {
        session.codex_queue.invalidate();
        session.codex_queue.error = None;
        update.clear_error = true;
    }
}

/// Ignore old-thread replies and retain all native fields in a complete queue snapshot.
pub fn reply(
    session: &mut SessionState,
    scope: CodexScope,
    update: &mut QueueUpdate,
    params: &Value,
) -> Option<(u64, Value)> {
    if params["requestParams"]["threadId"] != *session.thread_id()
        || params["requestContext"]["epoch"].as_u64() != Some(scope.epoch)
    {
        return None;
    }
    let token = params["requestContext"]["token"].as_u64()?;
    if params["requestMethod"] == "thread/queue/list" {
        return page(session, update, token, &params["response"]);
    }
    mutation(session, update, token, params);
    None
}

fn page(
    session: &mut SessionState,
    update: &mut QueueUpdate,
    token: u64,
    response: &Value,
) -> Option<(u64, Value)> {
    let read = session.codex_queue.read.as_ref()?;
    if token != read.token {
        return None;
    }
    if read.revision != session.codex_queue.revision {
        session.codex_queue.read = None;
        return None;
    }
    if let Some(error) = response.get("error") {
        read_failed(session, update, &error_message(error));
        return None;
    }
    let result = &response["result"];
    if let Err(error) = append_page(session, result) {
        read_failed(session, update, &error);
        return None;
    }
    let cursor = result.get("nextCursor").cloned().unwrap_or(Value::Null);
    if !cursor.is_null() {
        return Some((token, cursor));
    }
    let read = session.codex_queue.read.take().expect("queue read");
    session.codex_queue.entries = read.entries;
    session.codex_queue.known = true;
    session.codex_queue.error = None;
    update.clear_error = true;
    update.select_first = true;
    None
}

fn append_page(session: &mut SessionState, result: &Value) -> Result<(), String> {
    let data = result["data"].as_array().ok_or("Malformed queue page")?;
    let read = session.codex_queue.read.as_mut().expect("queue read");
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

fn mutation(session: &mut SessionState, update: &mut QueueUpdate, token: u64, params: &Value) {
    let Some(operation) = &session.codex_queue.operation else {
        return;
    };
    if operation.token != token
        || params["requestMethod"] != operation.method
        || params["requestParams"] != operation.params
    {
        return;
    }
    let operation = session
        .codex_queue
        .operation
        .take()
        .expect("queue mutation");
    let result = &params["response"]["result"];
    let error = mutation_error(&operation, &params["response"]);
    if let Some(error) = error {
        update.recovery = operation.submitted;
        super::queue::fail(session, update, &error);
    } else {
        session.codex_queue.notice = if operation.method == "thread/queue/add" {
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
            update.clear_edit = true;
        }
        session.codex_queue.error = None;
        update.clear_error = true;
    }
    session.codex_queue.invalidate();
}

fn read_failed(session: &mut SessionState, update: &mut QueueUpdate, error: &str) {
    session.codex_queue.read = None;
    session.codex_queue.known = false;
    session.codex_queue.error = Some(crate::session::text::safe_text(error));
    update.clear_error = true;
}

fn error_message(error: &Value) -> String {
    crate::session::text::safe_text(
        error["message"]
            .as_str()
            .unwrap_or("Codex queue request failed"),
    )
}

fn mutation_error(
    operation: &super::queue_state::QueueOperation,
    response: &Value,
) -> Option<String> {
    let result = &response["result"];
    response.get("error").map(error_message).or_else(|| {
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
    })
}
