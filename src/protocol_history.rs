//! Read complete app-server history across paginated and older full responses.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use crate::backend::BackendClient;
use crate::protocol_params::{extract_result_object, parse_thread_id_from_start_or_resume};

/// Fill a thread response with every turn, using IDs to merge overlapping pages.
pub(crate) fn load_thread_history(
    client: &dyn BackendClient,
    response: &str,
    timeout: Duration,
) -> Result<String> {
    let thread_id = parse_thread_id_from_start_or_resume(response)?;
    let mut parsed = extract_result_object(response)?;
    let mut turns = Vec::new();
    let mut positions = HashMap::new();
    let mut seen_cursors = HashSet::new();
    let mut cursor = Value::Null;
    loop {
        let page = client.call(
            "thread/turns/list",
            json!({"threadId": thread_id, "sortDirection": "asc", "itemsView": "full",
                   "limit": 100, "cursor": cursor}),
            timeout,
        )?;
        let page: Value = serde_json::from_str(&page).context("invalid turn history page")?;
        if let Some(error) = page.get("error") {
            if cursor.is_null() && is_unsupported_method(error) {
                return Ok(response.to_string());
            }
            bail!("read thread history: {error}");
        }
        let result = page.get("result").context("missing turn history result")?;
        for turn in result
            .get("data")
            .and_then(Value::as_array)
            .context("missing turns")?
        {
            let id = turn
                .get("id")
                .and_then(Value::as_str)
                .context("missing turn id")?;
            if let Some(index) = positions.get(id).copied() {
                turns[index] = turn.clone();
            } else {
                positions.insert(id.to_string(), turns.len());
                turns.push(turn.clone());
            }
        }
        cursor = result.get("nextCursor").cloned().unwrap_or(Value::Null);
        if cursor.is_null() {
            break;
        }
        if !seen_cursors.insert(cursor.to_string()) {
            bail!("app-server repeated a turn history cursor");
        }
    }
    parsed["result"]["thread"]["turns"] = Value::Array(turns);
    Ok(parsed.to_string())
}

/// Identify an older server that does not expose a newer protocol method.
pub(crate) fn is_unsupported_method(error: &Value) -> bool {
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_lowercase();
    error.get("code").and_then(Value::as_i64) == Some(-32601)
        || message.contains("unknown method")
        || message.contains("unsupported method")
        || message.contains("does not support")
}
