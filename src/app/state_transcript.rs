//! AppState impl: transcript/message manipulation, rendered lines, and selection.

use super::models::{Message, MessageKind, RenderedLine, Role};
use super::selection::{RenderedLineSource, Selection};
use super::state::AppState;
use super::transcript_render::{format_read_summary_with_count, parse_read_summary};

impl AppState {
    // --- Transcript mutation ---

    pub(super) fn mark_transcript_dirty(&mut self) {
        self.mark_transcript_dirty_from(0);
    }

    pub(super) fn mark_transcript_dirty_from(&mut self, idx: usize) {
        self.render_cache
            .mark_transcript_dirty_from(self.messages.len(), idx);
    }

    pub(super) fn sync_auto_follow_bottom(&mut self, max_scroll: usize) {
        self.viewport.sync_auto_follow_bottom(max_scroll);
    }

    pub(super) fn ensure_rendered_lines(
        &mut self,
        width: usize,
        hidden_user_message_idx: Option<usize>,
    ) {
        self.render_cache
            .ensure_rendered_lines(&self.messages, width, hidden_user_message_idx);
    }

    pub(super) fn append_message(&mut self, role: Role, text: impl Into<String>) -> usize {
        self.messages.push(Message {
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

    pub(super) fn append_diff_message(
        &mut self,
        role: Role,
        file_path: Option<String>,
        diff: impl Into<String>,
    ) -> usize {
        self.messages.push(Message {
            role,
            text: diff.into(),
            kind: MessageKind::Diff,
            file_path,
        });
        let idx = self.messages.len() - 1;
        self.mark_transcript_dirty_from(idx);
        idx
    }

    pub(super) fn has_agent_item_mapping(&self, item_id: &str) -> bool {
        self.agent_item_to_index.contains_key(item_id)
    }

    pub(super) fn put_agent_item_mapping(&mut self, item_id: &str, idx: usize) {
        self.agent_item_to_index.insert(item_id.to_string(), idx);
    }

    pub(super) fn ensure_item_placeholder(&mut self, item_id: &str, role: Role) -> bool {
        if self.has_agent_item_mapping(item_id) {
            return false;
        }
        let idx = self.append_message(role, String::new());
        self.put_agent_item_mapping(item_id, idx);
        true
    }

    pub(super) fn command_override(&self, call_id: &str) -> Option<String> {
        self.command_render_overrides.get(call_id).cloned()
    }

    pub(super) fn update_mapped_message(
        &mut self,
        item_id: &str,
        role: Role,
        text: Option<String>,
        kind: MessageKind,
        file_path: Option<String>,
    ) -> bool {
        let Some(idx) = self.agent_item_to_index.get(item_id).copied() else {
            return false;
        };
        self.update_message_at_index(idx, role, text, kind, file_path);
        true
    }

    pub(super) fn upsert_mapped_message(
        &mut self,
        item_id: &str,
        role: Role,
        text: String,
        kind: MessageKind,
        file_path: Option<String>,
    ) {
        if self.update_mapped_message(item_id, role, Some(text.clone()), kind, file_path.clone()) {
            return;
        }

        let idx = if kind == MessageKind::Diff {
            self.append_diff_message(role, file_path, text)
        } else {
            self.append_message(role, text)
        };
        self.put_agent_item_mapping(item_id, idx);
    }

    pub(super) fn upsert_agent_delta(&mut self, item_id: &str, delta: &str) {
        if let Some(idx) = self.agent_item_to_index.get(item_id).copied() {
            let mut changed = false;
            if let Some(msg) = self.messages.get_mut(idx) {
                if msg.kind != MessageKind::Plain {
                    msg.kind = MessageKind::Plain;
                    msg.file_path = None;
                    msg.text.clear();
                }
                msg.text.push_str(delta);
                changed = true;
            }
            if changed {
                self.mark_transcript_dirty_from(idx);
            }
            return;
        }

        let idx = self.append_message(Role::Assistant, delta);
        self.put_agent_item_mapping(item_id, idx);
    }

    pub(super) fn upsert_reasoning_summary_delta(&mut self, item_id: &str, delta: &str) {
        if let Some(idx) = self.agent_item_to_index.get(item_id).copied() {
            let mut changed = false;
            if let Some(msg) = self.messages.get_mut(idx) {
                if msg.kind != MessageKind::Plain {
                    msg.kind = MessageKind::Plain;
                    msg.file_path = None;
                    msg.text.clear();
                }
                msg.text.push_str(delta);
                msg.text = normalize_reasoning_summary_stream(&msg.text);
                changed = true;
            }
            if changed {
                self.mark_transcript_dirty_from(idx);
            }
            return;
        }

        let idx = self.append_message(Role::Reasoning, delta);
        self.put_agent_item_mapping(item_id, idx);
    }

    pub(super) fn upsert_turn_diff(&mut self, turn_id: &str, diff: &str) {
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

    pub(super) fn set_command_override(&mut self, call_id: &str, summary: String) {
        self.command_render_overrides
            .insert(call_id.to_string(), summary.clone());
        let _ = self.update_mapped_message(
            call_id,
            Role::ToolCall,
            Some(summary),
            MessageKind::Plain,
            None,
        );
    }

    pub(super) fn coalesce_successive_read_summary_at(&mut self, idx: usize) -> Option<usize> {
        if idx == 0 || idx >= self.messages.len() {
            return None;
        }

        let Some(current) = self.messages.get(idx) else {
            return None;
        };
        if current.role != Role::ToolCall
            || current.kind != MessageKind::Plain
            || current.text.contains('\n')
            || current.text.trim().is_empty()
        {
            return None;
        }
        let Some((current_path, current_count)) = parse_read_summary(&current.text) else {
            return None;
        };
        let current_path = current_path.to_string();

        let mut previous_idx = idx.saturating_sub(1);
        while previous_idx > 0 {
            let Some(previous) = self.messages.get(previous_idx) else {
                return None;
            };
            let empty_tool_shell = previous.role == Role::ToolCall
                && previous.kind == MessageKind::Plain
                && previous.text.trim().is_empty();
            if !empty_tool_shell {
                break;
            }
            previous_idx -= 1;
        }

        let Some(previous) = self.messages.get(previous_idx) else {
            return None;
        };
        if previous.role != Role::ToolCall
            || previous.kind != MessageKind::Plain
            || previous.text.contains('\n')
            || previous.text.trim().is_empty()
        {
            return None;
        }
        let Some((previous_path, previous_count)) = parse_read_summary(&previous.text) else {
            return None;
        };
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

    fn update_message_at_index(
        &mut self,
        idx: usize,
        role: Role,
        text: Option<String>,
        kind: MessageKind,
        file_path: Option<String>,
    ) {
        let mut changed = false;
        if let Some(msg) = self.messages.get_mut(idx) {
            msg.role = role;
            msg.kind = kind;
            msg.file_path = file_path;
            if let Some(text) = text {
                msg.text = text;
            }
            changed = true;
        }
        if !changed {
            return;
        }

        let dirty_from = if kind == MessageKind::Plain && role == Role::ToolCall {
            self.coalesce_successive_read_summary_at(idx).unwrap_or(idx)
        } else {
            idx
        };
        self.mark_transcript_dirty_from(dirty_from);
    }

    // --- Rendered lines & selection ---

    pub(super) fn rendered_line_count(&self) -> usize {
        self.render_cache.rendered_line_count()
    }

    pub(super) fn rendered_line_at(&self, idx: usize) -> Option<&RenderedLine> {
        self.render_cache.rendered_line_at(idx)
    }

    pub(super) fn ensure_rendered_range_materialized(&mut self, start_idx: usize, end_idx: usize) {
        self.render_cache
            .ensure_rendered_range_materialized(&self.messages, start_idx, end_idx);
    }

    #[cfg(test)]
    pub(super) fn snapshot_rendered_lines(&mut self) -> Vec<RenderedLine> {
        self.render_cache.snapshot_rendered_lines(&self.messages)
    }

    pub(super) fn selected_text(&mut self, selection: Selection) -> String {
        let start_idx = selection.anchor_line_idx.min(selection.focus_line_idx);
        let end_idx = selection.anchor_line_idx.max(selection.focus_line_idx);
        if self.render_cache.rendered_total_lines > 0 {
            self.ensure_rendered_range_materialized(
                start_idx.min(self.render_cache.rendered_total_lines.saturating_sub(1)),
                end_idx.min(self.render_cache.rendered_total_lines.saturating_sub(1)),
            );
        }
        super::selection::selected_text(selection, self)
    }
}

// --- Trait impl ---
impl RenderedLineSource for AppState {
    fn len(&self) -> usize {
        self.rendered_line_count()
    }

    fn get(&self, idx: usize) -> Option<&RenderedLine> {
        self.rendered_line_at(idx)
    }
}

// --- Free functions ---

pub(super) fn normalize_reasoning_summary_stream(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut i = 0usize;
    let bytes = text.as_bytes();
    let mut prev_was_bold_summary = false;

    while i < text.len() {
        if bytes[i..].starts_with(b"**") {
            i += 2;
            let start = i;
            while i < text.len() && !bytes[i..].starts_with(b"**") {
                i += 1;
            }
            if i < text.len() {
                let inner = text[start..i].trim_end_matches(' ');
                if prev_was_bold_summary {
                    out.push('\n');
                }
                out.push_str("**");
                out.push_str(inner);
                out.push_str("**");
                i += 2;
                prev_was_bold_summary = true;
                i = skip_whitespace_before_bold(text, i);
                continue;
            }

            if prev_was_bold_summary {
                out.push('\n');
            }
            out.push_str("**");
            out.push_str(&text[start..]);
            break;
        }

        let rest = &text[i..];
        let Some(ch) = rest.chars().next() else {
            break;
        };
        out.push(ch);
        i += ch.len_utf8();
        prev_was_bold_summary = false;
    }

    out
}

fn skip_whitespace_before_bold(text: &str, mut i: usize) -> usize {
    while i < text.len() {
        let rest = &text[i..];
        if rest.starts_with("**") {
            break;
        }
        let Some(ch) = rest.chars().next() else {
            break;
        };
        if !ch.is_whitespace() {
            break;
        }
        i += ch.len_utf8();
    }
    i
}
