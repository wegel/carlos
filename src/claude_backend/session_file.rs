//! Claude CLI session file parsing and DAG traversal.

// --- Imports ---

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::BufRead;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::Value;

use super::types::claude_project_dir_name;

// --- Public types ---

#[derive(Debug)]
pub(super) struct ClaudeSessionRecord {
    pub(super) record: Value,
    pub(super) uuid: Option<String>,
    pub(super) parent_uuid: Option<String>,
    pub(super) is_sidechain: bool,
}

// --- Record parsing ---

pub(super) fn user_record_text(record: &Value) -> Option<String> {
    let message = record.get("message").and_then(Value::as_object)?;
    if message.get("role").and_then(Value::as_str) != Some("user") {
        return None;
    }

    match message.get("content") {
        Some(Value::String(text)) => {
            let trimmed = text.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
        Some(Value::Array(parts)) => {
            let text_parts: Vec<&str> = parts
                .iter()
                .filter(|part| part.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|part| part.get("text").and_then(Value::as_str))
                .filter(|text| !text.trim().is_empty())
                .collect();
            (!text_parts.is_empty()).then(|| text_parts.join("\n"))
        }
        _ => None,
    }
}

fn parse_session_record(record: Value) -> ClaudeSessionRecord {
    ClaudeSessionRecord {
        uuid: record
            .get("uuid")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        parent_uuid: record
            .get("parentUuid")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        is_sidechain: record
            .get("isSidechain")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        record,
    }
}

pub(super) fn read_session_records_from_file(path: &Path) -> Result<Vec<ClaudeSessionRecord>> {
    let file = File::open(path)
        .with_context(|| format!("failed to open Claude session file {}", path.display()))?;
    let reader = std::io::BufReader::new(file);
    let mut saw_malformed_record = false;
    let mut records = Vec::new();

    for line in reader.lines() {
        let Ok(line) = line else {
            saw_malformed_record = true;
            continue;
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(record) = serde_json::from_str::<Value>(trimmed) else {
            saw_malformed_record = true;
            continue;
        };
        records.push(parse_session_record(record));
    }

    if saw_malformed_record {
        bail!("Claude session file contained malformed JSONL records");
    }

    Ok(records)
}

// --- Branch traversal ---

pub(super) fn active_branch_uuids(records: &[ClaudeSessionRecord]) -> Option<HashSet<String>> {
    let mut parent_by_uuid = HashMap::new();
    let mut active_tip_uuid = None;
    for record in records {
        if record.is_sidechain {
            continue;
        }
        let Some(uuid) = record.uuid.as_deref() else {
            continue;
        };
        parent_by_uuid.insert(uuid.to_string(), record.parent_uuid.clone());
        active_tip_uuid = Some(uuid.to_string());
    }

    let mut active_branch = HashSet::new();
    let mut cursor = active_tip_uuid?;
    loop {
        if !active_branch.insert(cursor.clone()) {
            break;
        }
        let Some(parent_uuid) = parent_by_uuid
            .get(&cursor)
            .and_then(|parent| parent.as_deref())
        else {
            break;
        };
        cursor = parent_uuid.to_string();
    }
    Some(active_branch)
}

// --- Session lookup ---

pub(super) fn find_session_file_for_resume(
    projects_root: &Path,
    cwd: &Path,
    session_id: &str,
) -> Option<PathBuf> {
    let file_name = format!("{session_id}.jsonl");
    let preferred = projects_root
        .join(claude_project_dir_name(cwd))
        .join(&file_name);
    if preferred.is_file() {
        return Some(preferred);
    }

    let entries = fs::read_dir(projects_root).ok()?;
    for entry in entries.flatten() {
        let candidate = entry.path().join(&file_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}
