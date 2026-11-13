//! Decodes bounded history chunks and emits one record's facts before its translated items.

use std::collections::VecDeque;

use anyhow::{bail, Context, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde_json::{json, Value};

use super::event_normalizer::fact;
use super::event_sources::{attach_sources, HistorySources};
use super::message_visibility::{is_child_message, is_child_report_notice};
use super::translate::translate_claude_line;
use super::types::ClaudeTranslationState;

/// Keeps native chunks, history translation, and paced normalized output separate from live work.
#[derive(Default)]
pub(super) struct HistoryDecoder {
    pub(super) session: Option<String>,
    translation: ClaudeTranslationState,
    sources: HistorySources,
    chunk: Vec<u8>,
    pub(super) delivery: VecDeque<String>,
}

impl HistoryDecoder {
    /// Translate a complete page or one bounded piece of a single large record.
    pub(super) fn page(&mut self, result: &Value, output: &mut Vec<String>) -> Result<()> {
        let records = if let Some(chunk) = result.get("chunk") {
            self.read_chunk(chunk)?.into_iter().collect()
        } else {
            result["messages"]
                .as_array()
                .context("SDK history page has no messages")?
                .clone()
        };
        for record in records {
            self.append_record(&record, output)?;
        }
        Ok(())
    }

    fn read_chunk(&mut self, chunk: &Value) -> Result<Option<Value>> {
        let offset = chunk["offset"]
            .as_u64()
            .context("History chunk has no offset")?;
        let total = chunk["totalBytes"]
            .as_u64()
            .context("History chunk has no size")?;
        let bytes = STANDARD.decode(
            chunk["data"]
                .as_str()
                .context("History chunk has no data")?,
        )?;
        if total > 16 * 1024 * 1024
            || offset != self.chunk.len() as u64
            || self.chunk.len() as u64 + bytes.len() as u64 > total
        {
            bail!("Claude history chunk exceeds its declared size or arrived out of order");
        }
        self.chunk.extend(bytes);
        if self.chunk.len() as u64 != total {
            return Ok(None);
        }
        let record = serde_json::from_slice(&self.chunk)?;
        self.chunk.clear();
        Ok(Some(record))
    }

    fn append_record(&mut self, record: &Value, output: &mut Vec<String>) -> Result<()> {
        let message = &record["message"];
        if message["session_id"].as_str() != self.session.as_deref() {
            bail!("Claude history belongs to a different session");
        }
        // Retain opaque native metadata for rewind/history inspection, never for translation by a client.
        output.push(fact("claude/historyRecord", json!({"record":message})));
        if is_child_message(message) || is_child_report_notice(message) {
            return Ok(());
        }
        if message["type"] == "user" {
            if let Some(text) = prompt_text(&message["message"]["content"]) {
                let mut prompt = json!({"source":record["source"].as_str().unwrap_or("unknown"),"inputKind":record["inputKind"],"uuid":message["uuid"],"text":text});
                if let Some(uuid) = message["uuid"].as_str().filter(|id| !id.is_empty()) {
                    prompt["sourceKey"] =
                        json!({"sessionId":message["session_id"],"kind":"user","id":uuid});
                }
                output.push(fact("claude/historyPrompt", prompt));
            }
        }
        if message["type"] == "system" && message["subtype"] == "compact_boundary" {
            output.push(fact("claude/historyCompacted", json!({})));
        }
        if !matches!(message["type"].as_str(), Some("user" | "assistant")) {
            return Ok(());
        }
        let base = if message["type"] == "assistant" {
            self.translation.begin_history_snapshot();
            self.sources.begin_record(message)
        } else {
            0
        };
        for line in translate_claude_line(&mut self.translation, &message.to_string())?.lines {
            let mut value: Value = serde_json::from_str(&line)?;
            if value["method"]
                .as_str()
                .is_some_and(|method| method.starts_with("item/"))
            {
                attach_sources(&self.translation, message, &mut value);
                if let Some(block) = value["params"]["sourceKey"]["block"].as_u64() {
                    value["params"]["sourceKey"]["block"] = json!(block + base as u64);
                }
                value["params"]["history"] = json!(true);
                output.push(value.to_string());
            }
        }
        Ok(())
    }
}

fn prompt_text(content: &Value) -> Option<String> {
    let text = content.as_str().map(str::to_string).or_else(|| {
        content.as_array().map(|parts| {
            parts
                .iter()
                .filter(|part| part["type"] == "text")
                .filter_map(|part| part["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n")
        })
    })?;
    (!text.trim().is_empty()).then_some(text)
}
