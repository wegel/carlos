//! Codex rollout fallback for app-server responses that omit later terminal turns.

use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{ensure, Context, Result};
use serde_json::Value;

use crate::protocol_params::extract_result_object;

#[path = "codex_rollout.rs"]
mod rollout;
pub(super) use rollout::parse_rollout_history;
use rollout::IneligibleRollout;

// --- Public Selection API ---

/// Choose complete local lifecycle history only when app-server omits a terminal turn.
pub(crate) fn preferred_codex_thread(response_line: &str, thread_id: &str) -> Result<Value> {
    let parsed = extract_result_object(response_line)?;
    let server_thread = parsed
        .get("result")
        .and_then(Value::as_object)
        .and_then(|result| result.get("thread"))
        .cloned()
        .context("missing thread in history response")?;
    if parsed["result"]["carlosHistorySource"] == "paged" {
        return Ok(server_thread);
    }
    let Some(rollout_thread) = load_rollout_thread(thread_id)? else {
        return Ok(server_thread);
    };
    prefer_complete_rollout_history(server_thread, rollout_thread)
}

/// Prefer rollout history when it contains a terminal turn missing from app-server.
pub(super) fn prefer_complete_rollout_history(mut server: Value, rollout: Value) -> Result<Value> {
    let Some(turns) = server["turns"].as_array_mut() else {
        return Ok(server);
    };
    let raw = rollout["turns"].as_array().cloned().unwrap_or_default();
    let positions: HashMap<String, usize> = turns
        .iter()
        .enumerate()
        .filter_map(|(index, turn)| turn["id"].as_str().map(|id| (id.into(), index)))
        .collect();
    let last_shared = raw.iter().rposition(|turn| {
        turn["id"]
            .as_str()
            .is_some_and(|id| positions.contains_key(id))
    });
    for (index, turn) in raw.into_iter().enumerate() {
        if let Some(target) = turn["id"]
            .as_str()
            .and_then(|id| positions.get(id))
            .copied()
        {
            merge_missing_items(&mut turns[target], &turn)?;
        } else if turn["status"] == "completed"
            && turn_has_final_answer(&turn)
            && last_shared.is_none_or(|last| index > last)
        {
            turns.push(turn);
        }
    }
    ensure!(
        server.to_string().len() <= 64 * 1024 * 1024,
        "Recovered Codex history exceeds 64 MiB; visible history was not replaced"
    );
    Ok(server)
}

fn merge_missing_items(server: &mut Value, raw: &Value) -> Result<()> {
    if server["status"] != "completed"
        || raw["status"] != "completed"
        || turn_has_final_answer(server)
        || !turn_has_final_answer(raw)
    {
        return Ok(());
    }
    let Some(items) = server["items"].as_array_mut() else {
        return Ok(());
    };
    let raw_items = raw["items"].as_array().expect("raw turn items");
    let raw_positions: HashMap<&str, usize> = raw_items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| item["id"].as_str().map(|id| (id, index)))
        .collect();
    let positions = items
        .iter()
        .filter_map(|item| item["id"].as_str().and_then(|id| raw_positions.get(id)))
        .copied()
        .collect::<Vec<_>>();
    ensure!(
        positions.windows(2).all(|pair| pair[0] < pair[1]),
        "Legacy and native Codex history disagree on item order; visible history was not replaced"
    );
    let ids: HashSet<&str> = raw_items
        .iter()
        .filter_map(|item| item["id"].as_str())
        .collect();
    let mut versions = HashMap::new();
    let mut gaps: HashMap<Option<String>, Vec<Value>> = HashMap::new();
    let mut previous = None;
    for item in std::mem::take(items) {
        if item["id"].as_str().is_some_and(|id| ids.contains(id)) {
            let id = item["id"].as_str().expect("item ID").to_string();
            previous = Some(id.clone());
            versions.insert(id, item);
        } else {
            gaps.entry(previous.clone()).or_default().push(item);
        }
    }
    items.extend(gaps.remove(&None).unwrap_or_default());
    for item in raw_items {
        let id = item["id"].as_str().expect("raw item ID");
        items.push(versions.remove(id).unwrap_or_else(|| item.clone()));
        items.extend(gaps.remove(&Some(id.into())).unwrap_or_default());
    }
    Ok(())
}

// --- Local File Discovery ---

fn load_rollout_thread(thread_id: &str) -> Result<Option<Value>> {
    let Some(root) = codex_sessions_root() else {
        return Ok(None);
    };
    let Some(path) = find_rollout_file(&root, thread_id)? else {
        return Ok(None);
    };
    read_rollout_file(&path, thread_id)
}

fn read_rollout_file(path: &Path, thread_id: &str) -> Result<Option<Value>> {
    let file =
        File::open(path).with_context(|| format!("open Codex rollout {}", path.display()))?;
    match parse_rollout_history(BufReader::new(file), thread_id) {
        Ok(history) => Ok(Some(history)),
        Err(error) if error.downcast_ref::<IneligibleRollout>().is_some() => Ok(None),
        Err(error) => Err(error)
            .with_context(|| format!("Read complete legacy Codex history {}", path.display())),
    }
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

#[cfg(test)]
#[path = "../tests/codex_history_file_tests.rs"]
mod tests;
