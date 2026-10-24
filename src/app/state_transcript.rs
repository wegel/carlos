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
        if self.render_cache.rendered_width != width
            || self.render_cache.rendered_compact != self.tool_output.compact()
        {
            self.viewport.selection = None;
            self.viewport.mouse_drag_mode = super::selection::MouseDragMode::Undecided;
        }
        let anchor = (!self.viewport.auto_follow_bottom)
            .then(|| self.render_cache.message_anchor(self.viewport.scroll_top))
            .flatten();
        self.render_cache.ensure_rendered_lines(
            &self.messages,
            width,
            hidden_user_message_idx,
            self.tool_output.compact(),
        );
        if let Some(line) = anchor.and_then(|anchor| self.render_cache.line_for_anchor(anchor)) {
            self.viewport.scroll_top = line;
        }
    }

    pub(super) fn append_message(&mut self, role: Role, text: impl Into<String>) -> usize {
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

    pub(super) fn append_diff_message(
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

    pub(super) fn command_override(&self, call_id: &str) -> Option<String> {
        self.command_render_overrides.get(call_id).cloned()
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
        if let Some(&idx) = self.item_to_message_index.get(call_id) {
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

    pub(super) fn coalesce_successive_read_summary_at(&mut self, idx: usize) -> Option<usize> {
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

    pub(super) fn update_message_at_index(
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
