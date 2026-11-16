//! Read one complete newest/older turn window while retaining the full ancestry reader.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};

use super::{
    load_thread_history, next_cursor, unsupported, validate_items, HistoryReader, MAX_RECORDS,
};
use crate::backend::BackendClient;
use crate::protocol_params::{extract_result_object, parse_thread_id_from_start_or_resume};
use crate::session::codex::history::preferred_codex_thread;

/// Address one older native window without exposing its cursor to a client.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HistoryCursor {
    pub(crate) thread: String,
    pub(crate) next: String,
}

/// Carry complete native turns and every cursor crossed while skipping empty turns.
pub(crate) struct HistoryPage {
    pub(crate) thread: Value,
    pub(crate) older: Option<HistoryCursor>,
    pub(crate) visited: Vec<String>,
    pub(crate) read_bytes: usize,
}

struct Window {
    turns: Vec<Value>,
    next: Value,
    visited: Vec<String>,
    legacy: bool,
}

/// Read newest complete turns while retaining full recovery for older native projections.
pub(crate) fn newest_window(
    client: &dyn BackendClient,
    response: &str,
    timeout: Duration,
) -> Result<HistoryPage> {
    let thread = parse_thread_id_from_start_or_resume(response)?;
    let mut parsed = extract_result_object(response)?;
    let mut reader = reader(client, thread.clone(), timeout);
    if parsed["result"]["thread"]["historyMode"] == "legacy" {
        return complete_compatibility(&reader, response);
    }
    let Some(window) = reader.full_window(Value::Null)? else {
        let complete = reader.legacy(parsed)?;
        return Ok(HistoryPage {
            thread: preferred_codex_thread(&complete, &thread)?,
            older: None,
            visited: Vec::new(),
            read_bytes: reader.bytes,
        });
    };
    // Older projections retain complete native/rollout recovery.
    if window.legacy {
        return complete_compatibility(&reader, response);
    }
    parsed["result"]["thread"]["turns"] = json!(window.turns);
    parsed["result"]["carlosHistorySource"] = json!("paged");
    let older = cursor(&thread, &window.next);
    let completed = reader.finish(parsed)?;
    Ok(HistoryPage {
        thread: preferred_codex_thread(&completed, &thread)?,
        older,
        visited: window.visited,
        read_bytes: reader.bytes,
    })
}

/// Read complete older turns through the original native continuation.
pub(crate) fn older_window(
    client: &dyn BackendClient,
    cursor: &HistoryCursor,
    timeout: Duration,
    read_bytes: usize,
) -> Result<HistoryPage> {
    let mut reader = reader(client, cursor.thread.clone(), timeout);
    reader.bytes = read_bytes;
    let window = reader
        .full_window(json!(cursor.next))?
        .context("Native history reader disappeared")?;
    ensure!(
        !window.legacy,
        "Native history lost its complete item projection"
    );
    let older = self::cursor(&cursor.thread, &window.next);
    let parsed = json!({"result":{"thread":{"id":cursor.thread,"turns":window.turns}}});
    let completed = reader.finish(parsed)?;
    Ok(HistoryPage {
        thread: extract_result_object(&completed)?["result"]["thread"].take(),
        older,
        visited: window.visited,
        read_bytes: reader.bytes,
    })
}

fn reader(client: &dyn BackendClient, thread: String, timeout: Duration) -> HistoryReader<'_> {
    HistoryReader {
        client,
        thread,
        deadline: Instant::now() + timeout,
        pages: 0,
        bytes: 0,
    }
}

fn complete_compatibility(reader: &HistoryReader<'_>, response: &str) -> Result<HistoryPage> {
    let complete = load_thread_history(
        reader.client,
        response,
        reader.deadline.saturating_duration_since(Instant::now()),
    )?;
    Ok(HistoryPage {
        thread: preferred_codex_thread(&complete, &reader.thread)?,
        older: None,
        visited: Vec::new(),
        read_bytes: reader.bytes,
    })
}

impl HistoryReader<'_> {
    fn full_window(&mut self, mut cursor: Value) -> Result<Option<Window>> {
        let mut seen = HashSet::new();
        if !cursor.is_null() {
            seen.insert(cursor.to_string());
        }
        let mut visited = Vec::new();
        let mut turns = Vec::new();
        let mut positions = HashMap::new();
        loop {
            let Some(page) = self.window_page(cursor)? else {
                return Ok(None);
            };
            let mut rows = page_turns(&page)?;
            let legacy = rows.iter().any(|turn| turn.get("itemsView").is_none())
                || !self.hydrate_window(&mut rows)?;
            let next = next_cursor(&page, &mut seen)?;
            if let Some(next) = next.as_str() {
                visited.push(next.to_owned());
            }
            let empty = rows
                .iter()
                .all(|turn| turn["items"].as_array().is_some_and(Vec::is_empty));
            for row in rows {
                let id = row["id"].as_str().expect("checked turn id").to_owned();
                if let Some(index) = positions.get(&id).copied() {
                    turns[index] = row;
                } else {
                    positions.insert(id, turns.len());
                    turns.push(row);
                }
            }
            ensure!(
                turns.len() <= MAX_RECORDS,
                "Native history window exceeds 500000 turns"
            );
            if legacy || !empty || next.is_null() {
                turns.reverse();
                return Ok(Some(Window {
                    turns,
                    next,
                    visited,
                    legacy,
                }));
            }
            cursor = next;
        }
    }

    fn window_page(&mut self, cursor: Value) -> Result<Option<Value>> {
        let first = cursor.is_null();
        let page = self.call(
            "thread/turns/list",
            json!({"threadId":self.thread,
            "sortDirection":"desc","itemsView":"full","limit":100,"cursor":cursor}),
        )?;
        Ok((!unsupported(&page, first)?).then_some(page))
    }

    fn hydrate_window(&mut self, turns: &mut [Value]) -> Result<bool> {
        for turn in turns {
            if turn["itemsView"] != "full" && turn.get("itemsView").is_some() {
                let id = turn["id"].as_str().context("Turn has no ID")?;
                let Some(items) = self.items(id)? else {
                    return Ok(false);
                };
                turn["items"] = json!(items);
                turn["itemsView"] = json!("full");
            }
            validate_items(turn)?;
        }
        Ok(true)
    }
}

fn page_turns(page: &Value) -> Result<Vec<Value>> {
    let rows = page["result"]["data"]
        .as_array()
        .context("Missing native turns")?;
    ensure!(
        rows.len() <= MAX_RECORDS,
        "Native history window exceeds 500000 turns"
    );
    let mut turns = Vec::with_capacity(rows.len());
    let mut positions = HashMap::new();
    for turn in rows {
        let id = turn["id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .context("Missing turn ID")?;
        if let Some(index) = positions.get(id).copied() {
            turns[index] = turn.clone();
        } else {
            positions.insert(id, turns.len());
            turns.push(turn.clone());
        }
    }
    Ok(turns)
}

fn cursor(thread: &str, next: &Value) -> Option<HistoryCursor> {
    next.as_str().map(|next| HistoryCursor {
        thread: thread.into(),
        next: next.into(),
    })
}

#[cfg(test)]
#[path = "tests/protocol_history_window_tests.rs"]
mod tests;
