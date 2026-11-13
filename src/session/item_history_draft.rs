//! Collect complete native history as plain drafts before allocating any Carlos ids.

use std::collections::HashMap;

use serde_json::Value;

use super::item_history::{
    agent_role_from_phase, has_terminal_item_status, reasoning_summary_text,
};
use crate::session::entry_key::BackendEntryKey;
use crate::session::history_identity::HistoryEntry;
use crate::session::input_history_native::NativeUserHistory;
use crate::session::models::EntryDraft;
use crate::session::models::{MessageKind, Role, TurnStatus};
use crate::session::native_activity::format_native_activity;
use crate::session::read_summary::{format_read_summary_with_count, parse_read_summary};
use crate::session::state::SessionState;
use crate::session::tools::{
    command_execution_diff_output, extract_diff_blocks, is_tool_call_type, is_tool_output_type,
    item_text_from_content,
};

#[path = "item_history_predecessors.rs"]
mod predecessors;
use predecessors::Predecessors;

/// Holds checked rows and native metadata without allocating entry ids.
pub(crate) struct HistoryDraft {
    pub(crate) entries: Vec<HistoryEntry>,
    pub(crate) records: Vec<HistoryRecord>,
    pub(crate) turns: Vec<(String, TurnStatus)>,
    #[cfg(test)]
    pub(crate) read_steps: usize,
}

/// Keep event-time row facts until the store has assigned ids to the whole history.
pub(crate) struct HistoryRecord {
    pub(crate) item: Value,
    pub(crate) turn: Option<String>,
    pub(crate) terminal: bool,
    pub(crate) updates: Vec<(usize, EntryDraft, bool)>,
    pub(crate) prompts: Vec<(usize, String)>,
    pub(crate) native_index: Option<usize>,
    pub(crate) observe: bool,
}

/// Collect every row before the live transcript adopts any of them.
pub(crate) fn build(session: &mut SessionState, thread: &Value) -> HistoryDraft {
    let mut draft = HistoryDraft {
        entries: Vec::new(),
        records: Vec::new(),
        turns: Vec::new(),
        #[cfg(test)]
        read_steps: 0,
    };
    let mut positions = HashMap::new();
    let mut predecessors = Predecessors::new();
    let mut native = NativeUserHistory::default();
    for turn in thread["turns"].as_array().into_iter().flatten() {
        session.replace_codex_item_turn(turn["id"].as_str().map(str::to_string));
        let status = turn["status"].as_str().and_then(TurnStatus::from_wire);
        if let (Some(id), Some(status)) = (turn["id"].as_str(), status) {
            draft.turns.push((id.to_string(), status));
        }
        for item in turn["items"].as_array().into_iter().flatten() {
            let mut record = HistoryRecord {
                item: item.clone(),
                turn: session.codex_item_turn().clone(),
                terminal: status.is_some_and(TurnStatus::is_terminal)
                    || has_terminal_item_status(item),
                updates: Vec::new(),
                prompts: Vec::new(),
                native_index: None,
                observe: false,
            };
            collect(
                session,
                &mut record,
                &mut draft.entries,
                &mut positions,
                &mut predecessors,
            );
            classify_native(
                session,
                &record,
                &mut native,
                &mut draft.entries,
                &mut predecessors,
            );
            draft.records.push(record);
        }
    }
    #[cfg(test)]
    {
        draft.read_steps = predecessors.steps;
    }
    draft
}

fn collect(
    session: &SessionState,
    record: &mut HistoryRecord,
    entries: &mut Vec<HistoryEntry>,
    positions: &mut HashMap<BackendEntryKey, usize>,
    predecessors: &mut Predecessors,
) {
    let item = &record.item;
    for (part, row) in rows(session, item).into_iter().enumerate() {
        let key = item["id"].as_str().map(|id| {
            let mut key = session.backend_entry_key(id);
            key.part = part;
            key
        });
        let inserted = key.as_ref().is_none_or(|key| !positions.contains_key(key));
        let describe = row.role == Role::ToolOutput || row.kind == MessageKind::Diff;
        let prompt = row.role == Role::User && inserted;
        let text = row.text.clone();
        let path = row.file_path.clone();
        let plain = row.kind == MessageKind::Plain;
        let index = upsert_row(entries, positions, key, row);
        let previous = coalesce(entries, index, predecessors);
        let row = &entries[index].draft;
        let shell = row.role == Role::ToolCall
            && row.kind == MessageKind::Plain
            && row.text.trim().is_empty();
        predecessors.set(index, !shell);
        if inserted && plain {
            entries[index].draft.file_path = path;
        }
        if let Some(previous) = previous {
            record
                .updates
                .push((previous, entries[previous].draft.clone(), false));
        }
        record
            .updates
            .push((index, entries[index].draft.clone(), describe));
        if prompt {
            record.prompts.push((index, text));
        }
    }
    record.native_index = item["id"]
        .as_str()
        .and_then(|id| positions.get(&session.backend_entry_key(id)))
        .copied();
    record.observe = record.native_index.is_some() && format_native_activity(item).is_none();
}

fn classify_native(
    session: &SessionState,
    record: &HistoryRecord,
    native: &mut NativeUserHistory,
    entries: &mut [HistoryEntry],
    predecessors: &mut Predecessors,
) {
    if record.item["type"] != "userMessage" || !session.input_history().native_enabled() {
        return;
    }
    if let (Some(item), Some(turn)) = (record.item["id"].as_str(), record.turn.as_deref()) {
        let position = native.record(item, turn, None);
        if let Some(index) = record.native_index {
            entries[index].draft.kind = if position == 0 {
                MessageKind::Plain
            } else {
                MessageKind::UserSteer
            };
            let row = &entries[index].draft;
            predecessors.set(
                index,
                !(row.role == Role::ToolCall
                    && row.kind == MessageKind::Plain
                    && row.text.trim().is_empty()),
            );
        }
    }
}

fn upsert_row(
    entries: &mut Vec<HistoryEntry>,
    positions: &mut HashMap<BackendEntryKey, usize>,
    key: Option<BackendEntryKey>,
    row: EntryDraft,
) -> usize {
    if let Some(index) = key.as_ref().and_then(|key| positions.get(key)).copied() {
        entries[index].occurrences += 1;
        entries[index].draft = row;
        index
    } else {
        let index = entries.len();
        if let Some(key) = &key {
            positions.insert(key.clone(), index);
        }
        entries.push(HistoryEntry {
            draft: row,
            key,
            occurrences: 1,
        });
        index
    }
}

/// Parse the same plain rows for full history and incremental reconnect records.
pub(crate) fn rows(session: &SessionState, item: &Value) -> Vec<EntryDraft> {
    if let Some(text) = format_native_activity(item) {
        return item["id"]
            .as_str()
            .map(|_| EntryDraft::plain(Role::ToolCall, text))
            .into_iter()
            .collect();
    }
    match item["type"].as_str().unwrap_or("") {
        "userMessage" => item_text_from_content(item)
            .map(|text| EntryDraft::plain(Role::User, text))
            .into_iter()
            .collect(),
        "agentMessage" => item["text"]
            .as_str()
            .map(str::to_string)
            .or_else(|| item_text_from_content(item))
            .map(|text| {
                EntryDraft::plain(
                    item.as_object()
                        .map(agent_role_from_phase)
                        .unwrap_or(Role::Assistant),
                    text,
                )
            })
            .into_iter()
            .collect(),
        "reasoning" => reasoning_summary_text(item)
            .map(|text| EntryDraft::plain(Role::Reasoning, text))
            .into_iter()
            .collect(),
        "contextCompaction" => vec![EntryDraft::plain(Role::System, "↻ Context compacted")],
        kind if kind == "commandExecution"
            || is_tool_call_type(kind)
            || is_tool_output_type(kind) =>
        {
            tool_rows(session, item, kind)
        }
        _ => Vec::new(),
    }
}

fn tool_rows(session: &SessionState, item: &Value, kind: &str) -> Vec<EntryDraft> {
    let role = if kind == "commandExecution" || is_tool_output_type(kind) {
        Role::ToolOutput
    } else {
        Role::ToolCall
    };
    let diffs = extract_diff_blocks(item);
    if !diffs.is_empty() {
        return diffs
            .into_iter()
            .map(|block| EntryDraft {
                role,
                text: block.diff,
                kind: MessageKind::Diff,
                file_path: block.file_path,
                tool_output: None,
            })
            .collect();
    }
    if role == Role::ToolOutput {
        if let Some(diff) = command_execution_diff_output(item) {
            return vec![EntryDraft {
                kind: MessageKind::Diff,
                ..EntryDraft::plain(role, diff)
            }];
        }
    }
    if let Some(formatted) = session
        .format_tool_item(item, role)
        .filter(|text| !text.is_empty())
    {
        return vec![EntryDraft {
            file_path: session.tool_source_path(item),
            ..EntryDraft::plain(role, formatted)
        }];
    }
    item["text"]
        .as_str()
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .or_else(|| item_text_from_content(item))
        .map(|text| EntryDraft::plain(role, text))
        .into_iter()
        .collect()
}

fn read(row: &EntryDraft) -> Option<(String, usize)> {
    if row.role != Role::ToolCall || row.kind != MessageKind::Plain || row.text.contains('\n') {
        return None;
    }
    parse_read_summary(&row.text).map(|(path, count)| (path.to_string(), count))
}

fn coalesce(
    entries: &mut [HistoryEntry],
    index: usize,
    predecessors: &mut Predecessors,
) -> Option<usize> {
    if index == 0 {
        return None;
    }
    let (path, count) = read(&entries[index].draft)?;
    let previous = predecessors.before(index).unwrap_or(0);
    let (old_path, old_count) = read(&entries[previous].draft)?;
    if path == old_path {
        entries[previous].draft.text = format_read_summary_with_count(&path, count + old_count);
        entries[index].draft.text.clear();
        entries[index].draft.kind = MessageKind::Plain;
        entries[index].draft.file_path = None;
        return Some(previous);
    }
    None
}

impl HistoryDraft {
    /// Scope draft keys to the native thread that will adopt them.
    pub(crate) fn set_conversation(&mut self, conversation: &str) {
        for entry in &mut self.entries {
            if let Some(key) = &mut entry.key {
                key.conversation = conversation.into();
            }
        }
    }
}
