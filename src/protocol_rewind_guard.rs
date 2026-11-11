//! Checks a selected Codex user item before any fork can remove native history.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use anyhow::{bail, ensure, Context, Result};
use serde_json::{json, Value};

use crate::backend::{BackendClient, RewindForkRequest, RewindUserMessage};
use crate::protocol_history::{is_unsupported_method, load_thread_history};
use crate::protocol_params::{extract_result_object, params_thread_read};

/// Validate exact native identity and reject steers before creating a new thread.
pub(crate) fn validate_rewind(
    client: &dyn BackendClient,
    thread_id: &str,
    request: &RewindForkRequest,
    timeout: Duration,
) -> Result<()> {
    let turn_id = request
        .before_turn_id
        .as_deref()
        .context("This prompt has no native turn ID; reload the session before editing it")?;
    let selected = request
        .before_user_item
        .as_ref()
        .context("This prompt has no exact native item ID; reload the session before editing it")?;
    if !validate_item_pages(client, thread_id, turn_id, selected, timeout)? {
        validate_full_history(client, thread_id, turn_id, selected, timeout)?;
    }
    Ok(())
}

fn validate_item_pages(
    client: &dyn BackendClient,
    thread_id: &str,
    turn_id: &str,
    selected: &RewindUserMessage,
    timeout: Duration,
) -> Result<bool> {
    let started = Instant::now();
    let mut cursor = Value::Null;
    let mut cursors = HashSet::new();
    let mut users = HashSet::new();
    loop {
        let remaining = timeout
            .checked_sub(started.elapsed())
            .context("Reading the selected turn timed out")?;
        let page = read_item_page(client, thread_id, turn_id, &cursor, remaining)?;
        if let Some(error) = page.get("error") {
            if cursor.is_null() && is_unsupported_method(error) {
                return Ok(false);
            }
            bail!("Read selected-turn history: {error}");
        }
        for entry in page["result"]["data"]
            .as_array()
            .context("Selected-turn history has no items")?
        {
            ensure!(
                entry["turnId"] == turn_id,
                "Selected-turn history returned a different turn"
            );
            if inspect_user(&entry["item"], selected, &mut users)? {
                return Ok(true);
            }
        }
        cursor = page["result"]["nextCursor"].clone();
        if cursor.is_null() {
            bail!("The selected prompt is missing from native history; reload the session");
        }
        ensure!(
            cursor.is_string() && cursors.insert(cursor.to_string()),
            "Native history repeated or returned an invalid item cursor"
        );
        ensure!(
            cursors.len() <= 4096 && users.len() <= 100000,
            "The selected turn exceeds the bounded history check"
        );
    }
}

fn read_item_page(
    client: &dyn BackendClient,
    thread_id: &str,
    turn_id: &str,
    cursor: &Value,
    timeout: Duration,
) -> Result<Value> {
    let response = client.call(
        "thread/items/list",
        json!({"threadId":thread_id,
        "turnId":turn_id,"sortDirection":"asc","limit":1,"cursor":cursor}),
        timeout,
    )?;
    serde_json::from_str(&response).context("Invalid selected-turn history")
}

fn validate_full_history(
    client: &dyn BackendClient,
    thread_id: &str,
    turn_id: &str,
    selected: &RewindUserMessage,
    timeout: Duration,
) -> Result<()> {
    let response = client.call("thread/read", params_thread_read(thread_id), timeout)?;
    let response = load_thread_history(client, &response, timeout)?;
    let parsed = extract_result_object(&response)?;
    ensure!(
        parsed["result"]["thread"]["id"] == thread_id,
        "Native history belongs to a different thread"
    );
    let turn = parsed["result"]["thread"]["turns"]
        .as_array()
        .context("Native history has no turns")?
        .iter()
        .find(|turn| turn["id"] == turn_id)
        .context("The selected turn is missing from native history; reload the session")?;
    let mut users = HashSet::new();
    for item in turn["items"]
        .as_array()
        .context("Native turn history has no items")?
    {
        if inspect_user(item, selected, &mut users)? {
            return Ok(());
        }
    }
    bail!("The selected prompt is missing from native history; reload the session")
}

fn inspect_user(
    item: &Value,
    selected: &RewindUserMessage,
    users: &mut HashSet<String>,
) -> Result<bool> {
    if item["type"] != "userMessage" {
        return Ok(false);
    }
    let id = item["id"]
        .as_str()
        .filter(|id| !id.is_empty())
        .context("Native user message has no item ID")?;
    let position = users.len();
    if !users.insert(id.to_string()) {
        return Ok(false);
    }
    if id != selected.item_id {
        return Ok(false);
    }
    ensure!(
        position == 0,
        "This message is a steer and cannot be edited independently"
    );
    ensure!(
        position == selected.position,
        "The selected prompt's native position changed; reload the session before editing it"
    );
    let content = item["content"]
        .as_array()
        .context("Native prompt has no content")?;
    let mut text = Vec::new();
    for part in content {
        ensure!(part["type"] == "text", "This prompt contains attachments or bindings that Carlos cannot safely restore for editing");
        text.push(
            part["text"]
                .as_str()
                .context("Native prompt has invalid text")?,
        );
    }
    ensure!(
        text.join("\n") == selected.text,
        "The selected prompt changed in native history; reload the session before editing it"
    );
    Ok(true)
}
