//! Change transcript facts and report the earliest row the view must rebuild.

use super::entry_id::EntryId;
use super::models::{EntryDraft, Message, MessageKind, Role};
use super::read_summary::{format_read_summary_with_count, parse_read_summary};
use super::state::SessionState;

/// A synchronous report consumed once by the caller after a session edit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SessionUpdate {
    /// Earliest changed entry, including a preceding coalesced read label.
    pub dirty_from: Option<EntryId>,
    /// Rebuild all rows after replacing the transcript.
    pub transcript_reset: bool,
}

impl SessionState {
    /// Report an existing row without doing layout or scanning the transcript.
    pub(super) fn dirty_entry(&self, index: usize) -> SessionUpdate {
        SessionUpdate {
            dirty_from: Some(self.messages[index].id),
            ..SessionUpdate::default()
        }
    }

    /// Append a plain row through the same read-group rules as completed items.
    pub fn append_message(
        &mut self,
        role: Role,
        text: impl Into<String>,
    ) -> (usize, SessionUpdate) {
        self.append_draft_message(EntryDraft::plain(role, text))
    }

    /// Assign an entry id before grouping read labels and reporting the earliest change.
    pub(crate) fn append_draft_message(&mut self, draft: EntryDraft) -> (usize, SessionUpdate) {
        self.messages.append(draft);
        let idx = self.messages.len() - 1;
        let dirty_from = self.coalesce_successive_read_summary_at(idx).unwrap_or(idx);
        (idx, self.dirty_entry(dirty_from))
    }

    /// Append a diff row without applying plain read-label grouping.
    pub fn append_diff_message(
        &mut self,
        role: Role,
        file_path: Option<String>,
        diff: impl Into<String>,
    ) -> (usize, SessionUpdate) {
        self.messages.append(EntryDraft {
            tool_output: None,
            role,
            text: diff.into(),
            kind: MessageKind::Diff,
            file_path,
        });
        let idx = self.messages.len() - 1;
        (idx, self.dirty_entry(idx))
    }

    /// Ignore empty or unchanged aggregate diffs; otherwise update or append one row.
    pub fn upsert_turn_diff(&mut self, turn_id: &str, diff: &str) -> SessionUpdate {
        if diff.trim().is_empty() {
            return SessionUpdate::default();
        }

        if let Some(idx) = self
            .messages
            .turn_diff(turn_id)
            .and_then(|id| self.messages.position(id))
        {
            if self.messages[idx].text == diff && self.messages[idx].kind == MessageKind::Diff {
                return SessionUpdate::default();
            }
            self.messages.update(
                idx,
                Role::ToolOutput,
                Some(diff.to_string()),
                MessageKind::Diff,
                None,
            );
            return self.dirty_entry(idx);
        }

        let (idx, update) = self.append_diff_message(Role::ToolOutput, None, diff.to_string());
        self.messages
            .bind_turn_diff(turn_id.to_string(), self.messages[idx].id);
        update
    }

    /// Combine matching read labels while keeping the later entry as a hidden shell.
    pub fn coalesce_successive_read_summary_at(&mut self, idx: usize) -> Option<usize> {
        if idx == 0 || idx >= self.messages.len() {
            return None;
        }
        let (current_path, current_count) = read_summary_at(self.messages.get(idx)?)?;
        let previous_idx = scan_back_past_empty_tool_shells(&self.messages, idx);
        let (previous_path, previous_count) = read_summary_at(self.messages.get(previous_idx)?)?;
        if previous_path != current_path {
            return None;
        }

        self.messages.update(
            previous_idx,
            Role::ToolCall,
            Some(format_read_summary_with_count(
                &current_path,
                previous_count + current_count,
            )),
            MessageKind::Plain,
            self.messages[previous_idx].file_path.clone(),
        );
        self.messages.update(
            idx,
            Role::ToolCall,
            Some(String::new()),
            MessageKind::Plain,
            None,
        );
        Some(previous_idx)
    }

    /// Ignore missing rows and preserve read grouping when replacing tool-call text.
    pub fn update_message_at_index(
        &mut self,
        idx: usize,
        role: Role,
        text: Option<String>,
        kind: MessageKind,
        file_path: Option<String>,
    ) -> SessionUpdate {
        if idx >= self.messages.len() {
            return SessionUpdate::default();
        }
        self.messages.update(idx, role, text, kind, file_path);

        let dirty_from = if kind == MessageKind::Plain && role == Role::ToolCall {
            self.coalesce_successive_read_summary_at(idx).unwrap_or(idx)
        } else {
            idx
        };
        self.dirty_entry(dirty_from)
    }
}

fn read_summary_at(message: &Message) -> Option<(String, usize)> {
    if message.role != Role::ToolCall
        || message.kind != MessageKind::Plain
        || message.text.contains('\n')
        || message.text.trim().is_empty()
    {
        return None;
    }
    let (path, count) = parse_read_summary(&message.text)?;
    Some((path.to_string(), count))
}

fn scan_back_past_empty_tool_shells(messages: &[Message], from_idx: usize) -> usize {
    let mut previous_idx = from_idx.saturating_sub(1);
    while previous_idx > 0 {
        let Some(previous) = messages.get(previous_idx) else {
            return previous_idx;
        };
        let empty_tool_shell = previous.role == Role::ToolCall
            && previous.kind == MessageKind::Plain
            && previous.text.trim().is_empty();
        if !empty_tool_shell {
            break;
        }
        previous_idx -= 1;
    }
    previous_idx
}
