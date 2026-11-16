//! Read one complete SDK archive window through its captured helper and original snapshot.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};

use super::event_history::HistoryDecoder;
use crate::backend::BackendClient;
use crate::protocol_history::MAX_HISTORY_BYTES;

#[derive(Clone, Debug, PartialEq, Eq)]
/// Bind one earlier prefix to its original native conversation and SDK snapshot.
pub(crate) struct HistoryCursor {
    pub(crate) thread: String,
    pub(crate) snapshot: String,
    pub(crate) end: u64,
}

/// Keep complete normalized facts until the owner can accept the entire prefix.
pub(crate) struct HistoryPage {
    pub(crate) lines: Vec<String>,
    pub(crate) older: Option<HistoryCursor>,
    pub(crate) read_bytes: usize,
}

#[derive(Default)]
struct WindowBytes {
    native: usize,
    facts: usize,
}

/// Finish every bounded native page/chunk before returning facts for atomic adoption.
pub(crate) fn older_window(
    client: &dyn BackendClient,
    cursor: &HistoryCursor,
    timeout: Duration,
    read_bytes: usize,
) -> Result<HistoryPage> {
    let deadline = Instant::now() + timeout;
    let mut current = cursor.clone();
    let mut newer = Vec::new();
    let mut bytes = WindowBytes {
        native: read_bytes,
        facts: 0,
    };
    loop {
        let (mut page, visible) = read_window(client, &current, deadline, &mut bytes)?;
        if visible || page.older.is_none() {
            for lines in newer.into_iter().rev() {
                page.lines.extend(lines);
            }
            return Ok(page);
        }
        current = page.older.expect("checked older window");
        newer.push(page.lines);
    }
}

fn read_window(
    client: &dyn BackendClient,
    cursor: &HistoryCursor,
    deadline: Instant,
    bytes: &mut WindowBytes,
) -> Result<(HistoryPage, bool)> {
    let mut params = json!({"sessionId":cursor.thread,"snapshotId":cursor.snapshot,
        "newest":true,"endOffset":cursor.end});
    let mut decoder = HistoryDecoder::default();
    decoder.session = Some(cursor.thread.clone());
    let mut lines = Vec::new();
    let mut seen = HashSet::new();
    let mut start = None;
    loop {
        ensure!(
            seen.insert(params.to_string()),
            "Claude history repeated a page or chunk"
        );
        let result = bytes.read(client, params.clone(), deadline)?;
        let result = &result["result"];
        let offset = checked_window(cursor, result, &mut start)?;
        ensure!(
            offset
                == params["offset"]
                    .as_u64()
                    .unwrap_or(start.expect("checked start")),
            "Claude history page does not match the requested offset"
        );
        let prior = lines.len();
        decoder.page(result, &mut lines)?;
        bytes.facts += lines[prior..].iter().map(String::len).sum::<usize>();
        ensure!(
            bytes.facts <= MAX_HISTORY_BYTES,
            "Claude history facts exceed 64 MiB"
        );
        if let Some((next, chunk)) = checked_progress(cursor, result, &decoder)? {
            params["offset"] = json!(next);
            params["chunkOffset"] = json!(chunk);
        } else {
            let older = start.filter(|start| *start > 0).map(|end| HistoryCursor {
                end,
                ..cursor.clone()
            });
            let page = HistoryPage {
                lines,
                older,
                read_bytes: bytes.native,
            };
            return Ok((page, decoder.has_entries));
        }
    }
}

impl WindowBytes {
    fn read(
        &mut self,
        client: &dyn BackendClient,
        params: Value,
        deadline: Instant,
    ) -> Result<Value> {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .context("Claude history read timed out")?;
        let reply = client.call("sessions.read", params, remaining)?;
        self.native = self.native.saturating_add(reply.len());
        ensure!(
            self.native <= MAX_HISTORY_BYTES,
            "Claude history window exceeds 64 MiB"
        );
        Ok(serde_json::from_str(&reply)?)
    }
}

fn checked_progress(
    cursor: &HistoryCursor,
    result: &Value,
    decoder: &HistoryDecoder,
) -> Result<Option<(u64, u64)>> {
    let offset = result["offset"]
        .as_u64()
        .context("Claude history has no page offset")?;
    if let Some(chunk) = decoder.pending_chunk_offset() {
        ensure!(
            result["chunkOffset"].as_u64() == Some(chunk)
                && result["nextOffset"].as_u64() == Some(offset),
            "Claude history chunk ended early or skipped native bytes"
        );
        return Ok(Some((offset, chunk)));
    }
    ensure!(
        result["chunkOffset"].is_null(),
        "Claude history continued a complete record"
    );
    let count = if result.get("chunk").is_some() {
        1
    } else {
        let messages = result["messages"]
            .as_array()
            .context("Claude history has no messages")?;
        ensure!(
            messages.len() <= 64,
            "Claude history page exceeds 64 records"
        );
        messages.len() as u64
    };
    let end = offset
        .checked_add(count)
        .context("Claude history offset overflow")?;
    ensure!(
        end > offset && end <= cursor.end,
        "Claude history page is empty or exceeds its window"
    );
    if end == cursor.end {
        ensure!(
            result["nextOffset"].is_null(),
            "Claude history continued past its window"
        );
        Ok(None)
    } else {
        ensure!(
            result["nextOffset"].as_u64() == Some(end),
            "Claude history skipped records or ended early"
        );
        Ok(Some((end, 0)))
    }
}

fn checked_window(cursor: &HistoryCursor, result: &Value, start: &mut Option<u64>) -> Result<u64> {
    ensure!(
        result["snapshotId"].as_str() == Some(&cursor.snapshot),
        "Claude history snapshot changed"
    );
    ensure!(
        result["windowEnd"].as_u64() == Some(cursor.end),
        "Claude history window end changed"
    );
    let actual = result["windowStart"]
        .as_u64()
        .context("Claude history has no window start")?;
    ensure!(
        actual < cursor.end && start.is_none_or(|start| start == actual),
        "Claude history window start changed"
    );
    *start = Some(actual);
    ensure!(
        result["olderOffset"]
            == if actual == 0 {
                Value::Null
            } else {
                json!(actual)
            },
        "Claude history returned a different older boundary"
    );
    let offset = result["offset"]
        .as_u64()
        .context("Claude history has no page offset")?;
    ensure!(
        offset >= actual && offset < cursor.end,
        "Claude history offset is outside its window"
    );
    Ok(offset)
}
