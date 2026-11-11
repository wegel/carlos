//! Read bounded legacy lifecycle records without changing native files.

use std::collections::HashMap;
use std::fmt;
use std::io::BufRead;

use anyhow::{Context, Result};
use serde_json::{json, Value};

#[derive(Debug)]
pub(super) struct IneligibleRollout;

impl fmt::Display for IneligibleRollout {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("use native history after a fork or history removal")
    }
}

impl std::error::Error for IneligibleRollout {}

/// Rebuild a thread from completed lifecycle records in rollout file order.
pub(in crate::app) fn parse_rollout_history(
    reader: impl BufRead,
    thread_id: &str,
) -> Result<Value> {
    let mut history = RolloutHistory::new(thread_id);
    let mut reader = reader;
    let mut bytes = 0usize;
    while let Some(line) = crate::protocol_reader::read_line(&mut reader)? {
        bytes += line.len();
        anyhow::ensure!(
            bytes <= 64 * 1024 * 1024,
            "Legacy Codex history exceeds 64 MiB"
        );
        let record: Value = serde_json::from_str(&line).context("Invalid legacy Codex record")?;
        // Removal markers make a locally replayed prefix unsafe to restore.
        if matches!(
            record
                .get("payload")
                .and_then(|payload| payload.get("type"))
                .and_then(Value::as_str),
            Some("thread_rolled_back" | "thread_reverted")
        ) || record["type"] == "history_base"
            || (record["type"] == "session_meta"
                && [
                    "forked_from_id",
                    "forked_from",
                    "history_base",
                    "forkedFromId",
                ]
                .iter()
                .any(|key| !record["payload"][key].is_null()))
        {
            return Err(IneligibleRollout.into());
        }
        history.apply_record(&record);
    }
    Ok(history.into_thread())
}

// --- Rollout Reduction ---

struct RolloutHistory {
    thread_id: String,
    turns: Vec<RolloutTurn>,
    turn_positions: HashMap<String, usize>,
    item_positions: HashMap<(String, String), (usize, usize)>,
    current_turn_id: Option<String>,
}

struct RolloutTurn {
    id: String,
    status: String,
    items: Vec<Value>,
}

impl RolloutHistory {
    fn new(thread_id: &str) -> Self {
        Self {
            thread_id: thread_id.to_string(),
            turns: Vec::new(),
            turn_positions: HashMap::new(),
            item_positions: HashMap::new(),
            current_turn_id: None,
        }
    }

    fn apply_record(&mut self, record: &Value) {
        if record.get("type").and_then(Value::as_str) != Some("event_msg") {
            return;
        }
        let Some(payload) = record.get("payload") else {
            return;
        };
        match payload.get("type").and_then(Value::as_str) {
            Some("task_started") => self.start_turn(payload),
            Some("task_complete") => self.end_turn(payload, "completed"),
            Some("turn_aborted") => self.end_turn(payload, "interrupted"),
            Some("task_failed") => self.end_turn(payload, "failed"),
            Some("item_completed") => self.complete_item(payload),
            _ => {}
        }
    }

    fn start_turn(&mut self, payload: &Value) {
        let Some(turn_id) = payload.get("turn_id").and_then(Value::as_str) else {
            return;
        };
        self.ensure_turn(turn_id);
        self.current_turn_id = Some(turn_id.to_string());
    }

    fn end_turn(&mut self, payload: &Value, status: &str) {
        let turn_id = payload
            .get("turn_id")
            .and_then(Value::as_str)
            .or(self.current_turn_id.as_deref());
        let Some(turn_id) = turn_id.map(ToOwned::to_owned) else {
            return;
        };
        let turn_index = self.ensure_turn(&turn_id);
        self.turns[turn_index].status = status.to_string();
        if self.current_turn_id.as_deref() == Some(&turn_id) {
            self.current_turn_id = None;
        }
    }

    fn complete_item(&mut self, payload: &Value) {
        let turn_id = payload
            .get("turn_id")
            .and_then(Value::as_str)
            .or(self.current_turn_id.as_deref());
        let (Some(turn_id), Some(item)) = (turn_id, payload.get("item")) else {
            return;
        };
        let (turn_id, mut item) = (turn_id.to_string(), item.clone());
        normalize_item_type(&mut item);
        let Some(item_id) = item
            .get("id")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
        else {
            return;
        };
        if let Some((turn_index, item_index)) = self
            .item_positions
            .get(&(turn_id.clone(), item_id.clone()))
            .copied()
        {
            self.turns[turn_index].items[item_index] = item;
            return;
        }
        let turn_index = self.ensure_turn(&turn_id);
        let item_index = self.turns[turn_index].items.len();
        self.turns[turn_index].items.push(item);
        self.item_positions
            .insert((turn_id, item_id), (turn_index, item_index));
    }

    fn ensure_turn(&mut self, turn_id: &str) -> usize {
        if let Some(index) = self.turn_positions.get(turn_id).copied() {
            return index;
        }
        let index = self.turns.len();
        self.turns.push(RolloutTurn {
            id: turn_id.to_string(),
            status: "inProgress".to_string(),
            items: Vec::new(),
        });
        self.turn_positions.insert(turn_id.to_string(), index);
        index
    }

    fn into_thread(self) -> Value {
        let turns: Vec<Value> = self
            .turns
            .into_iter()
            .map(|turn| json!({"id": turn.id, "status": turn.status, "items": turn.items}))
            .collect();
        json!({"id": self.thread_id, "turns": turns})
    }
}

fn normalize_item_type(item: &mut Value) {
    let Some(item) = item.as_object_mut() else {
        return;
    };
    let Some(kind) = item.get("type").and_then(Value::as_str) else {
        return;
    };
    let normalized = match kind {
        "UserMessage" => "userMessage",
        "AgentMessage" => "agentMessage",
        "Reasoning" => "reasoning",
        "CommandExecution" => "commandExecution",
        "FileChange" => "fileChange",
        "McpToolCall" => "mcpToolCall",
        "ContextCompaction" => "contextCompaction",
        "Extension" => "extension",
        _ => return,
    };
    item.insert("type".to_string(), Value::String(normalized.to_string()));
}
