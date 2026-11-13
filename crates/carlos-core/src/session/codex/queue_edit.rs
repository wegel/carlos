//! Build queue edits and explicit selected-entry operations from plain values.

use serde_json::{json, Value};

use super::queue::{mutate, CodexScope, QueueUpdate};
use crate::backend::BackendClient;
use crate::session::state::SessionState;

/// Reject stale drafts before constructing the same native update request.
pub fn save_edit(
    client: &dyn BackendClient,
    session: &mut SessionState,
    scope: CodexScope,
    update: &mut QueueUpdate,
    id: &str,
    original: &Value,
    text: String,
) -> Result<(), &'static str> {
    if !session.codex_queue.known
        || !session
            .codex_queue
            .entries
            .iter()
            .any(|entry| entry["id"] == id && entry["input"] == *original)
    {
        return Err("Queue changed; this draft is preserved. Refresh and reselect before saving");
    }
    let params = json!({"threadId":session.thread_id(),"queuedSubmissionId":id,
        "input":[{"type":"text","text":text,"text_elements":[]}]});
    mutate(
        client,
        session,
        scope,
        update,
        "thread/queue/update",
        params,
        None,
    );
    Ok(())
}

/// Use only an explicitly supplied current identity for delete, start or reorder.
pub fn operate(
    client: &dyn BackendClient,
    session: &mut SessionState,
    scope: CodexScope,
    update: &mut QueueUpdate,
    selected: Option<&str>,
    command: &str,
) -> Result<(), &'static str> {
    let id = selected
        .filter(|id| {
            session.codex_queue.known
                && session
                    .codex_queue
                    .entries
                    .iter()
                    .any(|entry| entry["id"] == *id)
        })
        .ok_or("Select a current queued message after the queue finishes refreshing")?;
    let (method, params) = if matches!(command, "up" | "down") {
        let mut ids = session
            .codex_queue
            .entries
            .iter()
            .map(|entry| entry["id"].as_str().expect("queue ID").to_string())
            .collect::<Vec<_>>();
        let index = ids
            .iter()
            .position(|entry| entry == id)
            .expect("selected entry");
        let next = if command == "up" {
            index.saturating_sub(1)
        } else {
            (index + 1).min(ids.len() - 1)
        };
        ids.swap(index, next);
        (
            "thread/queue/reorder",
            json!({"threadId":session.thread_id(),"queuedSubmissionIds":ids}),
        )
    } else {
        let method = if command == "start" {
            "thread/queue/start"
        } else {
            "thread/queue/delete"
        };
        (
            method,
            json!({"threadId":session.thread_id(),"queuedSubmissionId":id}),
        )
    };
    mutate(client, session, scope, update, method, params, None);
    Ok(())
}
