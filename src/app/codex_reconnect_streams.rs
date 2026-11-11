//! Refresh overlapping partial native items instead of guessing which deltas a snapshot includes.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use anyhow::{ensure, Result};
use serde_json::{json, Value};

use super::state::AppState;
use crate::backend::BackendClient;

#[derive(Default)]
/// Retain at most the unfinished turn's snapshot bodies, with one read in flight.
pub(super) struct Streams {
    items: HashMap<(String, String), PartialItem>,
    bytes: usize,
}

struct PartialItem {
    text: String,
    due: Option<Instant>,
    pending: Option<u64>,
    final_until: Option<Instant>,
}

impl Streams {
    /// Seed exact item identities; completed turns need no overlap cache.
    pub(super) fn from_thread(thread: &Value) -> Result<Self> {
        let mut streams = Self::default();
        for turn in thread["turns"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|turn| turn["status"] == "inProgress")
        {
            let Some(turn_id) = turn["id"].as_str() else {
                continue;
            };
            for item in turn["items"].as_array().into_iter().flatten() {
                let Some(id) = item["id"].as_str() else {
                    continue;
                };
                let Some(text) = text(item) else {
                    continue;
                };
                streams.bytes += text.len() + turn_id.len() + id.len();
                ensure!(
                    streams.bytes <= 64 * 1024 * 1024 && streams.items.len() < 500_000,
                    "Codex partial-item snapshot exceeds64 MiB or500000 items"
                );
                streams.items.insert(
                    (turn_id.into(), id.into()),
                    PartialItem {
                        text,
                        due: None,
                        pending: None,
                        final_until: None,
                    },
                );
            }
        }
        Ok(streams)
    }
}

/// Keep complete native events authoritative; never append an ambiguous delta to a snapshot.
pub(super) fn event(app: &mut AppState, event: &Value) -> bool {
    let params = &event["params"];
    if params["threadId"] != app.thread_id {
        return false;
    }
    let method = event["method"].as_str().unwrap_or("");
    let turn = params["turnId"].as_str().unwrap_or("");
    let item = params["itemId"]
        .as_str()
        .or(params["item"]["id"].as_str())
        .unwrap_or("");
    let key = (turn.to_string(), item.to_string());
    if method == "item/completed" {
        if !retain_final_read(app, &key, &params["item"]) {
            remove(app, &key);
        }
    } else if method == "item/started" && app.codex.connection.streams.items.contains_key(&key) {
        return true;
    } else if method.starts_with("item/") && params["delta"].is_string() {
        let previous = app.codex.item_turn.replace(turn.to_string());
        let completed = app.is_item_completed(item);
        app.codex.item_turn = previous;
        if completed {
            return true;
        }
        if let Some(item) = app.codex.connection.streams.items.get_mut(&key) {
            item.due.get_or_insert(Instant::now());
            return true;
        }
    } else if method == "turn/completed" {
        complete_turn(app, params);
    }
    false
}

fn complete_turn(app: &mut AppState, params: &Value) {
    let previous = app.codex.item_turn.take();
    app.codex.item_turn = params["turn"]["id"].as_str().map(str::to_string);
    let turn = &params["turn"];
    for item in turn["items"].as_array().into_iter().flatten() {
        if let (Some(turn), Some(id)) = (turn["id"].as_str(), item["id"].as_str()) {
            retain_final_read(app, &(turn.into(), id.into()), item);
        }
        let frame = json!({"threadId":app.thread_id,"turnId":turn["id"],"item":item});
        super::notification_items::handle_item_notification(
            app,
            "item/completed",
            frame.as_object().expect("item object"),
        );
    }
    let keys = app
        .codex
        .connection
        .streams
        .items
        .iter()
        .filter(|(id, item)| turn["id"] == id.0 && item.final_until.is_none())
        .map(|(key, _)| key.clone())
        .collect::<Vec<_>>();
    for key in keys {
        remove(app, &key);
    }
    app.codex.item_turn = previous;
}

fn retain_final_read(app: &mut AppState, key: &(String, String), item: &Value) -> bool {
    if !super::tool_output_items::command_output_missing(item) {
        return false;
    }
    let Some(partial) = app.codex.connection.streams.items.get_mut(key) else {
        return false;
    };
    if partial.final_until.is_none() {
        partial.final_until = Some(Instant::now() + Duration::from_secs(5));
        partial.pending = None; // Retire the older partial read before issuing a distinct final token.
        partial.due = Some(Instant::now());
    }
    true
}

/// Read one exact persisted item at a time; ordinary prompts remain independent.
pub(super) fn tick(client: &dyn BackendClient, app: &mut AppState, now: Instant) -> bool {
    let changed = expire(app, now);
    if !app.codex.enabled
        || app.codex.closed
        || app
            .codex
            .connection
            .streams
            .items
            .values()
            .any(|item| item.pending.is_some())
    {
        return changed;
    }
    let key = app
        .codex
        .connection
        .streams
        .items
        .iter()
        .find(|(_, item)| item.due.is_some_and(|due| now >= due))
        .map(|(key, _)| key.clone());
    let Some((turn, item)) = key else {
        return changed;
    };
    app.codex.serial = app.codex.serial.saturating_add(1);
    let token = app.codex.serial;
    let entry = app
        .codex
        .connection
        .streams
        .items
        .get_mut(&(turn.clone(), item.clone()))
        .expect("partial item");
    entry.pending = Some(token);
    entry.due = None;
    let result = client.request_control_with_context(
        "thread/items/read",
        json!({"threadId":app.thread_id,"turnId":turn,"itemIds":[item]}),
        json!({"operation":"reconnectItem","epoch":app.codex.epoch,"token":token,"timeoutMs":5000}),
    );
    if let Err(error) = result {
        app.codex
            .connection
            .streams
            .items
            .get_mut(&(turn, item))
            .expect("partial item")
            .pending = None;
        app.set_status(format!(
            "Could not refresh partial Codex output: {error}; waiting for its complete event"
        ));
    }
    true
}

/// A later completion retires the pending read; an older partial reply cannot overwrite it.
pub(super) fn reply(app: &mut AppState, params: &Value) -> bool {
    if params["requestContext"]["operation"] != "reconnectItem" {
        return false;
    }
    if params["requestContext"]["epoch"].as_u64() != Some(app.codex.epoch)
        || params["requestParams"]["threadId"] != app.thread_id
    {
        return true;
    }
    let key = (
        params["requestParams"]["turnId"]
            .as_str()
            .unwrap_or("")
            .to_string(),
        params["requestParams"]["itemIds"][0]
            .as_str()
            .unwrap_or("")
            .to_string(),
    );
    let Some(partial) = app.codex.connection.streams.items.get_mut(&key) else {
        return true;
    };
    if partial.pending != params["requestContext"]["token"].as_u64() {
        return true;
    }
    partial.pending = None;
    if !params["response"]["error"].is_null() {
        if partial.final_until.is_some() {
            partial.due = Some(Instant::now() + Duration::from_millis(500));
        }
        return true;
    }
    let entries = params["response"]["result"]["data"].as_array();
    let Some(entry) = entries.and_then(|rows| {
        rows.iter()
            .find(|row| row["turnId"] == key.0 && row["item"]["id"] == key.1)
    }) else {
        return true;
    };
    update_partial(app, &key, entry);
    true
}

fn update_partial(app: &mut AppState, key: &(String, String), entry: &Value) {
    let Some(partial) = app.codex.connection.streams.items.get(key) else {
        return;
    };
    if partial.final_until.is_some()
        && (!entry["completedAtMs"].is_i64()
            || super::tool_output_items::command_output_missing(&entry["item"]))
    {
        app.codex
            .connection
            .streams
            .items
            .get_mut(key)
            .expect("partial item")
            .due = Some(Instant::now() + Duration::from_millis(500));
        return;
    }
    let Some(next) = text(&entry["item"]) else {
        return;
    };
    if !next.starts_with(&partial.text) {
        return;
    }
    let growth = next.len().saturating_sub(partial.text.len());
    if app.codex.connection.streams.bytes + growth > 64 * 1024 * 1024 {
        return;
    }
    let terminal = entry["completedAtMs"].is_i64();
    let partial = app
        .codex
        .connection
        .streams
        .items
        .get_mut(key)
        .expect("partial item");
    partial.text = next;
    partial.due = Some(Instant::now() + Duration::from_millis(500));
    app.codex.connection.streams.bytes += growth;
    let previous = app.codex.item_turn.replace(key.0.clone());
    if terminal {
        let frame = json!({"threadId":app.thread_id,"turnId":key.0,"item":entry["item"]});
        super::notification_items::handle_item_notification(
            app,
            "item/completed",
            frame.as_object().expect("item object"),
        );
        remove(app, key);
    } else {
        super::item_history::append_history_item(app, &entry["item"]);
    }
    app.codex.item_turn = previous;
}

/// Wake only for a partial item that actually overlapped streamed output.
pub(super) fn wait(app: &AppState, now: Instant) -> Option<Duration> {
    if app.codex.closed {
        return None;
    }
    let items = &app.codex.connection.streams.items;
    let pending = items.values().any(|item| item.pending.is_some());
    items
        .values()
        .flat_map(|item| {
            item.final_until
                .into_iter()
                .chain(item.due.filter(|_| !pending))
        })
        .map(|at| at.saturating_duration_since(now))
        .min()
}

fn expire(app: &mut AppState, now: Instant) -> bool {
    let expired = app
        .codex
        .connection
        .streams
        .items
        .iter()
        .filter(|(_, item)| item.final_until.is_some_and(|at| now >= at))
        .map(|(key, _)| key.clone())
        .collect::<Vec<_>>();
    for key in &expired {
        remove(app, key);
        app.append_message(super::Role::System, format!("Codex did not return complete output for item {} after reconnect. The last native snapshot is kept.",
            super::claude_question_state::safe_label(&key.1)));
    }
    !expired.is_empty()
}

fn remove(app: &mut AppState, key: &(String, String)) {
    if let Some(item) = app.codex.connection.streams.items.remove(key) {
        app.codex.connection.streams.bytes = app
            .codex
            .connection
            .streams
            .bytes
            .saturating_sub(item.text.len() + key.0.len() + key.1.len());
    }
}

fn text(item: &Value) -> Option<String> {
    match item["type"].as_str()? {
        "agentMessage" => item["text"].as_str().map(str::to_string),
        "commandExecution" => Some(item["aggregatedOutput"].as_str().unwrap_or("").into()),
        "reasoning" => super::item_history::reasoning_summary_text(item),
        _ => None,
    }
}
