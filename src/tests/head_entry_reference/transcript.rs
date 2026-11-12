//! Unchanged HEAD row updates; only imports and method visibility are adapted.

use super::models::{Message, MessageKind, Role};
use super::state::AppState;
pub(super) use crate::app::state_transcript::normalize_reasoning_summary_stream;
use crate::app::transcript_render::{format_read_summary_with_count, parse_read_summary};

impl AppState {
    pub(crate) fn append_message(&mut self, role: Role, text: impl Into<String>) -> usize {
        self.messages.push(Message {
            tool_output: None,
            role,
            text: text.into(),
            kind: MessageKind::Plain,
            file_path: None,
        });
        let idx = self.messages.len() - 1;
        let dirty_from = self.coalesce_successive_read_summary_at(idx).unwrap_or(idx);
        self.mark_transcript_dirty_from(dirty_from);
        idx
    }

    pub(crate) fn append_diff_message(
        &mut self,
        role: Role,
        file_path: Option<String>,
        diff: impl Into<String>,
    ) -> usize {
        self.messages.push(Message {
            tool_output: None,
            role,
            text: diff.into(),
            kind: MessageKind::Diff,
            file_path,
        });
        let idx = self.messages.len() - 1;
        self.mark_transcript_dirty_from(idx);
        idx
    }

    pub(crate) fn command_override(&self, call_id: &str) -> Option<String> {
        self.command_render_overrides
            .get(&self.item_key(call_id))
            .cloned()
    }

    pub(crate) fn upsert_turn_diff(&mut self, turn_id: &str, diff: &str) {
        if diff.trim().is_empty() {
            return;
        }

        if let Some(idx) = self.turn_diff_to_index.get(turn_id).copied() {
            if let Some(msg) = self.messages.get_mut(idx) {
                if msg.text == diff && msg.kind == MessageKind::Diff {
                    return;
                }
                msg.role = Role::ToolOutput;
                msg.text = diff.to_string();
                msg.kind = MessageKind::Diff;
                msg.file_path = None;
                self.mark_transcript_dirty_from(idx);
                return;
            }
        }

        let idx = self.append_diff_message(Role::ToolOutput, None, diff.to_string());
        self.turn_diff_to_index.insert(turn_id.to_string(), idx);
    }

    pub(crate) fn set_command_override(&mut self, call_id: &str, summary: String) {
        self.command_render_overrides
            .insert(self.item_key(call_id), summary.clone());
        if let Some(idx) = self.item_message_index(call_id) {
            if let Some(tool) = self.messages[idx].tool_output.as_mut() {
                tool.summary = summary;
                self.mark_transcript_dirty_from(idx);
                return;
            }
        }
        let _ = self.update_mapped_message(
            call_id,
            Role::ToolCall,
            Some(summary),
            MessageKind::Plain,
            None,
        );
    }

    pub(crate) fn coalesce_successive_read_summary_at(&mut self, idx: usize) -> Option<usize> {
        if idx == 0 || idx >= self.messages.len() {
            return None;
        }
        let (current_path, current_count) = read_summary_at(self.messages.get(idx)?)?;
        let previous_idx = scan_back_past_empty_tool_shells(&self.messages, idx);
        let (previous_path, previous_count) = read_summary_at(self.messages.get(previous_idx)?)?;
        if previous_path != current_path {
            return None;
        }

        if let Some(prev_msg) = self.messages.get_mut(previous_idx) {
            prev_msg.text =
                format_read_summary_with_count(&current_path, previous_count + current_count);
        }
        if let Some(current_msg) = self.messages.get_mut(idx) {
            current_msg.text.clear();
            current_msg.kind = MessageKind::Plain;
            current_msg.file_path = None;
        }
        Some(previous_idx)
    }

    pub(crate) fn update_message_at_index(
        &mut self,
        idx: usize,
        role: Role,
        text: Option<String>,
        kind: MessageKind,
        file_path: Option<String>,
    ) {
        let Some(msg) = self.messages.get_mut(idx) else {
            return;
        };
        msg.role = role;
        msg.kind = kind;
        msg.file_path = file_path;
        if let Some(text) = text {
            msg.text = text;
        }

        let dirty_from = if kind == MessageKind::Plain && role == Role::ToolCall {
            self.coalesce_successive_read_summary_at(idx).unwrap_or(idx)
        } else {
            idx
        };
        self.mark_transcript_dirty_from(dirty_from);
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
