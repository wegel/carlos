//! Codex rollout fallback for app-server responses that omit later terminal turns.

use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result};
use serde_json::{json, Value};

use crate::protocol_params::extract_result_object;

// --- Public Selection API ---

/// Choose complete local lifecycle history only when app-server omits a terminal turn.
pub(super) fn preferred_codex_thread(response_line: &str, thread_id: &str) -> Result<Value> {
    let parsed = extract_result_object(response_line)?;
    let server_thread = parsed
        .get("result")
        .and_then(Value::as_object)
        .and_then(|result| result.get("thread"))
        .cloned()
        .context("missing thread in history response")?;
    let Ok(Some(rollout_thread)) = load_rollout_thread(thread_id) else {
        return Ok(server_thread);
    };
    Ok(prefer_complete_rollout_history(
        server_thread,
        rollout_thread,
    ))
}

/// Prefer rollout history when it contains a terminal turn missing from app-server.
pub(super) fn prefer_complete_rollout_history(server: Value, rollout: Value) -> Value {
    if rollout_has_missing_terminal_turn(&server, &rollout) {
        rollout
    } else {
        server
    }
}

/// Rebuild a thread from completed lifecycle records in rollout file order.
pub(super) fn parse_rollout_history(reader: impl BufRead, thread_id: &str) -> Result<Value> {
    let mut history = RolloutHistory::new(thread_id);
    for line in reader.lines() {
        let line = line.context("read Codex rollout record")?;
        let Ok(record) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        // Removal markers make a locally replayed prefix unsafe to restore.
        anyhow::ensure!(
            !matches!(
                record
                    .get("payload")
                    .and_then(|payload| payload.get("type"))
                    .and_then(Value::as_str),
                Some("thread_rolled_back" | "thread_reverted")
            ),
            "use app-server history after a thread history removal"
        );
        history.apply_record(&record);
    }
    Ok(history.into_thread())
}

// --- Local File Discovery ---

fn load_rollout_thread(thread_id: &str) -> Result<Option<Value>> {
    let Some(root) = codex_sessions_root() else {
        return Ok(None);
    };
    let Some(path) = find_rollout_file(&root, thread_id)? else {
        return Ok(None);
    };
    let file =
        File::open(&path).with_context(|| format!("open Codex rollout {}", path.display()))?;
    parse_rollout_history(BufReader::new(file), thread_id).map(Some)
}

fn codex_sessions_root() -> Option<PathBuf> {
    env::var_os("CODEX_HOME")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|path| PathBuf::from(path).join(".codex")))
        .map(|path| path.join("sessions"))
}

fn find_rollout_file(root: &Path, thread_id: &str) -> Result<Option<PathBuf>> {
    if !root.is_dir() {
        return Ok(None);
    }
    let suffix = format!("-{thread_id}.jsonl");
    let mut matches = Vec::new();
    collect_rollout_matches(root, &suffix, 4, &mut matches)?;
    matches.sort_by_key(|path| {
        fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH)
    });
    Ok(matches.pop())
}

fn collect_rollout_matches(
    directory: &Path,
    suffix: &str,
    remaining_depth: usize,
    matches: &mut Vec<PathBuf>,
) -> Result<()> {
    for entry in fs::read_dir(directory)
        .with_context(|| format!("read Codex sessions directory {}", directory.display()))?
    {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() && remaining_depth > 0 {
            collect_rollout_matches(&entry.path(), suffix, remaining_depth - 1, matches)?;
        } else if file_type.is_file() && entry.file_name().to_string_lossy().ends_with(suffix) {
            matches.push(entry.path());
        }
    }
    Ok(())
}

// --- Completeness Check ---

fn rollout_has_missing_terminal_turn(server: &Value, rollout: &Value) -> bool {
    let server_ids = turn_ids(server);
    rollout
        .get("turns")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|turn| {
            turn.get("status").and_then(Value::as_str) == Some("completed")
                && turn_has_final_answer(turn)
        })
        .filter_map(|turn| turn.get("id").and_then(Value::as_str))
        .any(|turn_id| !server_ids.contains(turn_id))
}

fn turn_has_final_answer(turn: &Value) -> bool {
    turn.get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|item| {
            item.get("type").and_then(Value::as_str) == Some("agentMessage")
                && item.get("phase").and_then(Value::as_str) == Some("final_answer")
        })
}

fn turn_ids(thread: &Value) -> HashSet<&str> {
    thread
        .get("turns")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|turn| turn.get("id").and_then(Value::as_str))
        .collect()
}

// --- Rollout Reduction ---

struct RolloutHistory {
    thread_id: String,
    turns: Vec<RolloutTurn>,
    turn_positions: HashMap<String, usize>,
    item_positions: HashMap<String, (usize, usize)>,
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
        if let Some((turn_index, item_index)) = self.item_positions.get(&item_id).copied() {
            self.turns[turn_index].items[item_index] = item;
            return;
        }
        let turn_index = self.ensure_turn(&turn_id);
        let item_index = self.turns[turn_index].items.len();
        self.turns[turn_index].items.push(item);
        self.item_positions
            .insert(item_id, (turn_index, item_index));
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
