//! Assemble bounded authoritative history from public turn and item pages.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use anyhow::{bail, ensure, Context, Result};
use serde_json::{json, Value};

use crate::backend::BackendClient;
use crate::protocol_params::{extract_result_object, parse_thread_id_from_start_or_resume};

const MAX_HISTORY_BYTES: usize = 64 * 1024 * 1024;
const MAX_RECORDS: usize = 500_000;
const MAX_PAGES: usize = 65536;

struct HistoryReader<'a> {
    client: &'a dyn BackendClient,
    thread: String,
    deadline: Instant,
    pages: usize,
    bytes: usize,
}

/// Load every page before the caller replaces its visible transcript.
pub(crate) fn load_thread_history(
    client: &dyn BackendClient,
    response: &str,
    timeout: Duration,
) -> Result<String> {
    let mut reader = HistoryReader {
        client,
        thread: parse_thread_id_from_start_or_resume(response)?,
        deadline: Instant::now() + timeout,
        pages: 0,
        bytes: 0,
    };
    let mut parsed = extract_result_object(response)?;
    let Some(mut turns) = reader.turns("full")? else {
        return reader.legacy(parsed);
    };
    // Full turn pages avoid a native lineage/database lookup for every individual item.
    // A server that ignores the projection still gets the older item-page hydration below.
    let mut legacy = parsed["result"]["thread"]["historyMode"] == "legacy";
    let mut full_pages_needed = false;
    for turn in &mut turns {
        // Older public readers return full items without an itemsView discriminator.
        if turn.get("itemsView").is_none() || turn["itemsView"] == "full" {
            legacy |= turn.get("itemsView").is_none();
            validate_items(turn)?;
            continue;
        }
        let id = turn["id"].as_str().context("Turn has no ID")?;
        match reader.items(id)? {
            Some(items) => {
                turn["items"] = json!(items);
                turn["itemsView"] = json!("full");
            }
            None => {
                legacy = true;
                full_pages_needed = true;
                break;
            }
        }
    }
    if full_pages_needed {
        turns = reader
            .turns("full")?
            .context("Native turn reader disappeared")?;
        for turn in &mut turns {
            ensure!(
                turn.get("itemsView").is_none() || turn["itemsView"] == "full",
                "Native turn reader did not return full history"
            );
            validate_items(turn)?;
        }
    }
    parsed["result"]["thread"]["turns"] = json!(turns);
    parsed["result"]["carlosHistorySource"] = json!(if legacy { "legacy" } else { "paged" });
    reader.finish(parsed)
}

impl HistoryReader<'_> {
    fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        ensure!(self.pages < MAX_PAGES, "Codex history exceeds 65536 pages");
        self.pages += 1;
        let remaining = self
            .deadline
            .checked_duration_since(Instant::now())
            .context("Codex history read timed out")?;
        let response = self.client.call(method, params, remaining)?;
        self.bytes = self.bytes.saturating_add(response.len());
        ensure!(
            self.bytes <= MAX_HISTORY_BYTES,
            "Codex history exceeds 64 MiB; visible history was not replaced"
        );
        serde_json::from_str(&response).context("Invalid Codex history page")
    }

    fn turns(&mut self, view: &str) -> Result<Option<Vec<Value>>> {
        let mut turns = Vec::new();
        let mut positions = HashMap::new();
        let mut cursors = HashSet::new();
        let mut cursor = Value::Null;
        loop {
            let page = self.call(
                "thread/turns/list",
                json!({"threadId":self.thread,
                "sortDirection":"asc","itemsView":view,"limit":100,"cursor":cursor}),
            )?;
            if unsupported(&page, cursor.is_null())? {
                return Ok(None);
            }
            for turn in page["result"]["data"]
                .as_array()
                .context("Missing native turns")?
            {
                let id = turn["id"]
                    .as_str()
                    .filter(|id| !id.is_empty())
                    .context("Missing turn ID")?;
                if let Some(index) = positions.get(id).copied() {
                    turns[index] = turn.clone();
                } else {
                    positions.insert(id.to_string(), turns.len());
                    turns.push(turn.clone());
                }
                ensure!(
                    turns.len() <= MAX_RECORDS,
                    "Codex history exceeds 500000 turns"
                );
            }
            cursor = next_cursor(&page, &mut cursors)?;
            if cursor.is_null() {
                break;
            }
        }
        Ok(Some(turns))
    }

    fn items(&mut self, turn: &str) -> Result<Option<Vec<Value>>> {
        let mut items = Vec::new();
        let mut positions = HashMap::new();
        let mut cursors = HashSet::new();
        let mut cursor = Value::Null;
        loop {
            let page = self.call(
                "thread/items/list",
                json!({"threadId":self.thread,"turnId":turn,
                "sortDirection":"asc","limit":1,"cursor":cursor}),
            )?;
            if unsupported(&page, cursor.is_null())? {
                return Ok(None);
            }
            for entry in page["result"]["data"]
                .as_array()
                .context("Missing native items")?
            {
                ensure!(
                    entry["turnId"] == turn,
                    "Native history returned an item from another turn"
                );
                let item = &entry["item"];
                let id = item["id"]
                    .as_str()
                    .filter(|id| !id.is_empty())
                    .context("Missing item ID")?;
                if let Some(index) = positions.get(id).copied() {
                    items[index] = item.clone();
                } else {
                    positions.insert(id.to_string(), items.len());
                    items.push(item.clone());
                }
                ensure!(
                    items.len() <= MAX_RECORDS,
                    "Codex turn exceeds 500000 items"
                );
            }
            cursor = next_cursor(&page, &mut cursors)?;
            if cursor.is_null() {
                break;
            }
        }
        Ok(Some(items))
    }

    fn legacy(&mut self, mut parsed: Value) -> Result<String> {
        // Full-history reads are reserved for a server that lacks public turn pages.
        let response = self.call(
            "thread/read",
            json!({"threadId":self.thread,"includeTurns":true}),
        )?;
        ensure!(
            response["error"].is_null(),
            "Read older Codex history: {}",
            native_error(&response["error"])
        );
        ensure!(
            response["result"]["thread"]["id"] == self.thread,
            "Native history belongs to another thread"
        );
        let mut thread = response["result"]["thread"].clone();
        let turns = thread["turns"]
            .as_array_mut()
            .context("Legacy history has no turns")?;
        ensure!(
            turns.len() <= MAX_RECORDS,
            "Codex history exceeds 500000 turns"
        );
        for turn in turns {
            ensure!(
                turn["id"].as_str().is_some_and(|id| !id.is_empty()),
                "Missing turn ID"
            );
            validate_items(turn)?;
        }
        parsed["result"]["thread"] = thread;
        parsed["result"]["carlosHistorySource"] = json!("legacy");
        self.finish(parsed)
    }

    fn finish(&self, parsed: Value) -> Result<String> {
        ensure!(
            Instant::now() <= self.deadline,
            "Codex history read timed out; visible history was not replaced"
        );
        let serialized = parsed.to_string();
        ensure!(
            serialized.len() <= MAX_HISTORY_BYTES,
            "Codex history exceeds 64 MiB; visible history was not replaced"
        );
        Ok(serialized)
    }
}

#[cfg(test)]
#[path = "tests/protocol_history_tests.rs"]
mod tests;

fn unsupported(page: &Value, first: bool) -> Result<bool> {
    let Some(error) = page.get("error") else {
        return Ok(false);
    };
    if first && is_unsupported_method(error) {
        return Ok(true);
    }
    bail!("Read Codex history: {}", native_error(error))
}

fn next_cursor(page: &Value, seen: &mut HashSet<String>) -> Result<Value> {
    let cursor = page["result"]["nextCursor"].clone();
    if !cursor.is_null() {
        ensure!(
            cursor.is_string() && seen.insert(cursor.to_string()),
            "Native history repeated or returned an invalid cursor"
        );
    }
    Ok(cursor)
}

/// Recognize an older public server without guessing from unrelated failures.
pub(crate) fn is_unsupported_method(error: &Value) -> bool {
    let message = error["message"].as_str().unwrap_or("").to_lowercase();
    error["code"].as_i64() == Some(-32601)
        || message.starts_with("unknown method:")
        || message.starts_with("unsupported method:")
}

fn validate_items(turn: &mut Value) -> Result<()> {
    let items = turn["items"]
        .as_array_mut()
        .context("Full native turn has no item array")?;
    ensure!(
        items.len() <= MAX_RECORDS,
        "Codex turn exceeds 500000 items"
    );
    let mut positions = HashMap::new();
    let mut retained = Vec::with_capacity(items.len());
    for item in std::mem::take(items) {
        let id = item["id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .context("Missing item ID")?;
        if let Some(index) = positions.get(id).copied() {
            retained[index] = item;
        } else {
            positions.insert(id.to_string(), retained.len());
            retained.push(item);
        }
    }
    *items = retained;
    Ok(())
}

fn native_error(error: &Value) -> String {
    let message = error["message"]
        .as_str()
        .unwrap_or("Native history read failed");
    let message = crate::protocol_events::bounded_diagnostic(message);
    match error["code"].as_i64() {
        Some(code) => format!("({code}) {message}"),
        None => message,
    }
}
