//! Claude CLI session forking from transcript prefixes.

// --- Imports ---

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::BufRead;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::Value;
use uuid::Uuid;

use super::session_file::{
    active_branch_uuids, find_session_file_for_resume, read_session_records_from_file,
    user_record_text, ClaudeSessionRecord,
};
use super::types::claude_project_dir_name;

// --- Public types ---

#[derive(Debug)]
pub(super) struct ForkedClaudeSession {
    pub(super) session_id: String,
    pub(super) path: PathBuf,
}

#[derive(Debug)]
struct ClaudeVisibleMessage {
    uuid: String,
    is_user_prompt: bool,
}

// --- Branch prefix selection ---

fn visible_active_branch_messages(
    records: &[ClaudeSessionRecord],
    active_branch_uuids: Option<&HashSet<String>>,
) -> Vec<ClaudeVisibleMessage> {
    let mut visible = Vec::new();
    for record in records {
        if record.is_sidechain {
            continue;
        }
        let Some(uuid) = record.uuid.as_deref() else {
            continue;
        };
        if let Some(active_branch_uuids) = active_branch_uuids {
            if !active_branch_uuids.contains(uuid) {
                continue;
            }
        }
        match record.record.get("type").and_then(Value::as_str) {
            Some("user") => visible.push(ClaudeVisibleMessage {
                uuid: uuid.to_string(),
                is_user_prompt: user_record_text(&record.record).is_some(),
            }),
            Some("assistant") => visible.push(ClaudeVisibleMessage {
                uuid: uuid.to_string(),
                is_user_prompt: false,
            }),
            _ => {}
        }
    }
    visible
}

fn prefix_uuid_chain(
    records: &[ClaudeSessionRecord],
    up_to_message_uuid: Option<&str>,
) -> HashSet<String> {
    let Some(mut cursor) = up_to_message_uuid.map(str::to_string) else {
        return HashSet::new();
    };

    let mut parent_by_uuid = HashMap::new();
    for record in records {
        if record.is_sidechain {
            continue;
        }
        let Some(uuid) = record.uuid.as_deref() else {
            continue;
        };
        parent_by_uuid.insert(uuid.to_string(), record.parent_uuid.clone());
    }

    let mut kept = HashSet::new();
    loop {
        if !kept.insert(cursor.clone()) {
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
    kept
}

fn session_prefix_message_uuid(records: &[ClaudeSessionRecord], keep_turns: usize) -> Option<String> {
    let active_branch_uuids = active_branch_uuids(records);
    let visible = visible_active_branch_messages(records, active_branch_uuids.as_ref());
    let mut kept_turns = 0usize;
    let mut last_visible_uuid = None;

    for message in visible {
        if message.is_user_prompt {
            if kept_turns >= keep_turns {
                break;
            }
            kept_turns += 1;
        }
        last_visible_uuid = Some(message.uuid);
    }

    last_visible_uuid
}

// --- Transcript rewriting ---

fn rewrite_session_id(value: &mut Value, new_session_id: &str) {
    match value {
        Value::Object(map) => {
            if let Some(session_id) = map.get_mut("sessionId") {
                *session_id = Value::String(new_session_id.to_string());
            }
            for child in map.values_mut() {
                rewrite_session_id(child, new_session_id);
            }
        }
        Value::Array(items) => {
            for child in items {
                rewrite_session_id(child, new_session_id);
            }
        }
        _ => {}
    }
}

fn rewrite_uuid_fields(
    value: &mut Value,
    new_session_id: &str,
    uuid_map: &HashMap<String, String>,
) {
    match value {
        Value::Object(map) => {
            if let Some(session_id) = map.get_mut("sessionId") {
                *session_id = Value::String(new_session_id.to_string());
            }
            if let Some(uuid) = map.get("uuid").and_then(Value::as_str) {
                if let Some(remapped) = uuid_map.get(uuid).cloned() {
                    map.insert("uuid".to_string(), Value::String(remapped));
                }
            }
            if let Some(parent_uuid) = map.get("parentUuid").and_then(Value::as_str) {
                if let Some(remapped) = uuid_map.get(parent_uuid).cloned() {
                    map.insert("parentUuid".to_string(), Value::String(remapped));
                }
            }
            for child in map.values_mut() {
                rewrite_uuid_fields(child, new_session_id, uuid_map);
            }
        }
        Value::Array(items) => {
            for child in items {
                rewrite_uuid_fields(child, new_session_id, uuid_map);
            }
        }
        _ => {}
    }
}

fn copy_session_support_tree(
    source_root: &Path,
    dest_root: &Path,
    new_session_id: &str,
) -> Result<()> {
    if !source_root.is_dir() {
        return Ok(());
    }
    fs::create_dir_all(dest_root)
        .with_context(|| format!("failed to create {}", dest_root.display()))?;
    for entry in fs::read_dir(source_root)
        .with_context(|| format!("failed to read {}", source_root.display()))?
    {
        let entry = entry?;
        let source_path = entry.path();
        let dest_path = dest_root.join(entry.file_name());
        if source_path.is_dir() {
            copy_session_support_tree(&source_path, &dest_path, new_session_id)?;
            continue;
        }
        if source_path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
            let file = File::open(&source_path)
                .with_context(|| format!("failed to open {}", source_path.display()))?;
            let reader = std::io::BufReader::new(file);
            let mut lines = Vec::new();
            for line in reader.lines() {
                let line = line
                    .with_context(|| format!("failed to read {}", source_path.display()))?;
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    lines.push(String::new());
                    continue;
                }
                let mut record: Value = serde_json::from_str(trimmed)
                    .with_context(|| format!("invalid JSONL in {}", source_path.display()))?;
                rewrite_session_id(&mut record, new_session_id);
                lines.push(record.to_string());
            }
            let body = if lines.is_empty() {
                String::new()
            } else {
                format!("{}\n", lines.join("\n"))
            };
            fs::write(&dest_path, body)
                .with_context(|| format!("failed to write {}", dest_path.display()))?;
            continue;
        }
        fs::copy(&source_path, &dest_path).with_context(|| {
            format!(
                "failed to copy {} to {}",
                source_path.display(),
                dest_path.display()
            )
        })?;
    }
    Ok(())
}

// --- Public API ---

pub(super) fn fork_claude_session_from_projects_root(
    projects_root: &Path,
    cwd: &Path,
    source_session_id: &str,
    keep_turns: usize,
) -> Result<ForkedClaudeSession> {
    let source_session_path = find_session_file_for_resume(projects_root, cwd, source_session_id)
        .with_context(|| format!("Claude session not found: {source_session_id}"))?;
    let records = read_session_records_from_file(&source_session_path)?;
    let up_to_message_uuid = session_prefix_message_uuid(&records, keep_turns);
    let kept_uuid_chain = prefix_uuid_chain(&records, up_to_message_uuid.as_deref());

    let new_session_id = Uuid::new_v4().to_string();
    let project_dir = projects_root.join(claude_project_dir_name(cwd));
    fs::create_dir_all(&project_dir)
        .with_context(|| format!("failed to create {}", project_dir.display()))?;
    let dest_session_path = project_dir.join(format!("{new_session_id}.jsonl"));

    let mut uuid_map = HashMap::new();
    for record in &records {
        let Some(uuid) = record.uuid.as_deref() else {
            continue;
        };
        if kept_uuid_chain.contains(uuid) {
            uuid_map.insert(uuid.to_string(), Uuid::new_v4().to_string());
        }
    }

    let mut leading_metadata = true;
    let mut lines = Vec::new();
    for record in records {
        if record.is_sidechain {
            continue;
        }

        match record.uuid.as_deref() {
            Some(uuid) if kept_uuid_chain.contains(uuid) => {
                leading_metadata = false;
                let mut rewritten = record.record;
                rewrite_uuid_fields(&mut rewritten, &new_session_id, &uuid_map);
                lines.push(rewritten.to_string());
            }
            Some(_) => {
                leading_metadata = false;
            }
            None if leading_metadata => {
                let mut rewritten = record.record;
                rewrite_session_id(&mut rewritten, &new_session_id);
                lines.push(rewritten.to_string());
            }
            None => {}
        }
    }

    let body = if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    };
    fs::write(&dest_session_path, body)
        .with_context(|| format!("failed to write {}", dest_session_path.display()))?;

    copy_session_support_tree(
        &source_session_path.with_extension(""),
        &dest_session_path.with_extension(""),
        &new_session_id,
    )?;

    Ok(ForkedClaudeSession {
        session_id: new_session_id,
        path: dest_session_path,
    })
}
