//! Carries a large normalized history fact in bounded pieces without exposing native chunks.

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use super::event_normalizer::fact;

const PART_BYTES: usize = 128 * 1024;
const MAX_FACT_BYTES: usize = 64 * 1024 * 1024;

/// Buffers only normalized JSON; native history decoding and translator cursors stay in the backend.
#[derive(Default)]
pub(crate) struct FactTransfer {
    buffer: String,
}

impl FactTransfer {
    /// Accept one ordered fragment and return its complete normalized fact when ready.
    pub(crate) fn receive(&mut self, params: &Value) -> Result<Option<Value>> {
        let offset = params["offset"]
            .as_u64()
            .context("Normalized history part has no offset")?;
        let total = params["totalBytes"]
            .as_u64()
            .context("Normalized history part has no size")?;
        let text = params["text"]
            .as_str()
            .context("Normalized history part has no text")?;
        if total > MAX_FACT_BYTES as u64
            || offset != self.buffer.len() as u64
            || offset + text.len() as u64 > total
        {
            bail!("Normalized history fact exceeds its size or arrived out of order");
        }
        self.buffer.push_str(text);
        if self.buffer.len() as u64 != total {
            return Ok(None);
        }
        let mut value: Value = serde_json::from_str(&self.buffer)?;
        self.buffer.clear();
        value["params"]["generation"] = params["generation"].clone();
        Ok(Some(value))
    }
}

/// Keep small facts unchanged and split larger facts along UTF-8 boundaries.
pub(super) fn parts(line: String) -> Result<Vec<String>> {
    if line.len() <= PART_BYTES {
        return Ok(vec![line]);
    }
    if line.len() > MAX_FACT_BYTES {
        bail!("Normalized history fact exceeds 64 MiB");
    }
    let value: Value = serde_json::from_str(&line)?;
    let mut offset = 0;
    let mut output = Vec::new();
    while offset < line.len() {
        let mut end = (offset + PART_BYTES).min(line.len());
        while !line.is_char_boundary(end) {
            end -= 1;
        }
        output.push(fact("claude/historyFactPart", json!({"offset":offset,"totalBytes":line.len(),"text":&line[offset..end],"epoch":value["params"]["epoch"]})));
        offset = end;
    }
    Ok(output)
}
