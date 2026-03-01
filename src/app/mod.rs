use std::collections::VecDeque;
use std::collections::{HashMap, HashSet};
use std::env;
use std::io;
use std::sync::mpsc::{RecvTimeoutError, TryRecvError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyCode, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags, MouseButton,
    MouseEventKind, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;
use ratatui::Terminal;
use ratatui_core::style::{Color as CoreColor, Modifier as CoreModifier, Style as CoreStyle};
use ratatui_interact::components::{
    DiffData, DiffViewMode, DiffViewer, DiffViewerState, DiffViewerStyle,
};
use ratatui_textarea::{Input as TextInput, Key as TextKey, TextArea};
use serde_json::{json, Value};
use tui_markdown::{
    from_str_with_options as markdown_from_str_with_options, Options as MarkdownOptions,
};

use self::perf::{DurationSamples, PerfMetrics};
use self::selection::{
    compute_selection_range, decide_mouse_drag_mode, normalize_selection_x, selected_text,
    MouseDragMode, Selection,
};
use self::text::{
    char_to_byte_idx, slice_by_cells, split_at_cells, visual_width, wrap_input_line,
    wrap_natural_by_cells,
};
use crate::clipboard::*;
use crate::event::{spawn_event_forwarders, UiEvent};
use crate::protocol::*;
use crate::theme::*;

mod perf;
mod selection;
mod text;

const MSG_TOP: usize = 1; // 1-based row index
const MSG_CONTENT_X: usize = 2; // 0-based x

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Role {
    User,
    Assistant,
    Reasoning,
    ToolCall,
    ToolOutput,
    System,
}

#[derive(Debug, Clone)]
struct Message {
    role: Role,
    text: String,
    kind: MessageKind,
    file_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MessageKind {
    Plain,
    Diff,
}

#[derive(Debug, Clone)]
struct DiffBlock {
    file_path: Option<String>,
    diff: String,
}

#[derive(Debug, Clone)]
struct ThreadSummary {
    id: String,
    preview: String,
    cwd: String,
    updated_at: i64,
}

#[derive(Debug, Clone)]
struct StyledSegment {
    text: String,
    style: Style,
}

#[derive(Debug, Clone)]
struct RenderedLine {
    text: String,
    styled_segments: Vec<StyledSegment>,
    role: Role,
    separator: bool,
    cells: usize,
    soft_wrap_to_next: bool,
}

#[derive(Debug, Clone, Copy)]
struct TerminalSize {
    width: usize,
    height: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ContextUsage {
    used: u64,
    max: u64,
}

#[derive(Debug)]
struct AppState {
    thread_id: String,
    active_turn_id: Option<String>,
    messages: Vec<Message>,
    rendered_lines: Vec<RenderedLine>,
    rendered_width: usize,
    rendered_hidden_user_message_idx: Option<usize>,
    transcript_dirty: bool,
    agent_item_to_index: HashMap<String, usize>,
    turn_diff_to_index: HashMap<String, usize>,
    command_render_overrides: HashMap<String, String>,

    input: TextArea<'static>,
    input_history: Vec<String>,
    input_history_message_idx: Vec<Option<usize>>,
    input_history_index: Option<usize>,
    input_history_draft: Option<String>,
    rewind_mode: bool,
    rewind_restore_draft: Option<String>,
    esc_armed_at: Option<Instant>,
    status: String,

    scroll_top: usize,
    auto_follow_bottom: bool,
    selection: Option<Selection>,
    mouse_drag_mode: MouseDragMode,
    mouse_drag_last_row: usize,
    mobile_mouse_buffer: String,
    mobile_mouse_last_y: Option<usize>,
    show_help: bool,
    context_usage: Option<ContextUsage>,
    perf: Option<PerfMetrics>,
}

impl AppState {
    fn new(thread_id: String) -> Self {
        Self {
            thread_id,
            active_turn_id: None,
            messages: Vec::new(),
            rendered_lines: Vec::new(),
            rendered_width: 0,
            rendered_hidden_user_message_idx: None,
            transcript_dirty: true,
            agent_item_to_index: HashMap::new(),
            turn_diff_to_index: HashMap::new(),
            command_render_overrides: HashMap::new(),
            input: make_input_area(),
            input_history: Vec::new(),
            input_history_message_idx: Vec::new(),
            input_history_index: None,
            input_history_draft: None,
            rewind_mode: false,
            rewind_restore_draft: None,
            esc_armed_at: None,
            status: String::new(),
            scroll_top: 0,
            auto_follow_bottom: true,
            selection: None,
            mouse_drag_mode: MouseDragMode::Undecided,
            mouse_drag_last_row: 0,
            mobile_mouse_buffer: String::new(),
            mobile_mouse_last_y: None,
            show_help: false,
            context_usage: None,
            perf: None,
        }
    }

    fn enable_perf_metrics(&mut self) {
        self.perf = Some(PerfMetrics::new());
    }

    fn perf_report(&self) -> Option<String> {
        self.perf.as_ref().map(PerfMetrics::final_report)
    }

    fn set_status(&mut self, s: impl Into<String>) {
        self.status = s.into();
    }

    fn input_is_empty(&self) -> bool {
        self.input.is_empty()
    }

    fn input_text(&self) -> String {
        self.input.lines().join("\n")
    }

    fn clear_input(&mut self) {
        self.input = make_input_area();
        self.reset_input_history_navigation();
    }

    fn set_input_text(&mut self, text: &str) {
        self.input = make_input_area();
        if !text.is_empty() {
            let _ = self.input.insert_str(text.to_string());
        }
    }

    fn reset_input_history_navigation(&mut self) {
        self.input_history_index = None;
        self.input_history_draft = None;
    }

    fn reset_esc_chord(&mut self) {
        self.esc_armed_at = None;
    }

    fn expire_esc_chord(&mut self, now: Instant) {
        const ESC_CHORD_WINDOW: Duration = Duration::from_millis(700);
        if let Some(armed_at) = self.esc_armed_at {
            if now.duration_since(armed_at) > ESC_CHORD_WINDOW {
                self.esc_armed_at = None;
            }
        }
    }

    fn register_escape_press(&mut self, now: Instant) -> bool {
        const ESC_CHORD_WINDOW: Duration = Duration::from_millis(700);
        if let Some(armed_at) = self.esc_armed_at {
            if now.duration_since(armed_at) <= ESC_CHORD_WINDOW {
                self.esc_armed_at = None;
                return true;
            }
        }
        self.esc_armed_at = Some(now);
        false
    }

    fn enter_rewind_mode(&mut self) {
        if self.rewind_mode {
            return;
        }
        self.rewind_mode = true;
        self.auto_follow_bottom = false;
        self.rewind_restore_draft = Some(self.input_text());
        self.reset_input_history_navigation();
        let _ = self.navigate_input_history_up();
    }

    fn exit_rewind_mode_restore(&mut self) {
        if !self.rewind_mode {
            return;
        }
        let draft = self.rewind_restore_draft.take().unwrap_or_default();
        self.rewind_mode = false;
        self.auto_follow_bottom = true;
        self.set_input_text(&draft);
        self.reset_input_history_navigation();
    }

    fn clear_rewind_mode_state(&mut self) {
        self.rewind_mode = false;
        self.auto_follow_bottom = true;
        self.rewind_restore_draft = None;
    }

    fn push_input_history(&mut self, text: &str) {
        self.record_input_history(text, None);
    }

    fn record_input_history(&mut self, text: &str, message_idx: Option<usize>) {
        if text.is_empty() {
            self.reset_input_history_navigation();
            return;
        }

        if let Some(msg_idx) = message_idx {
            if let (Some(last_text), Some(last_idx)) = (
                self.input_history.last(),
                self.input_history_message_idx.last_mut(),
            ) {
                if *last_text == text && last_idx.is_none() {
                    *last_idx = Some(msg_idx);
                    self.reset_input_history_navigation();
                    return;
                }
            }
        }

        self.input_history.push(text.to_string());
        self.input_history_message_idx.push(message_idx);
        self.reset_input_history_navigation();
    }

    fn navigate_input_history_up(&mut self) -> bool {
        if self.input_history.is_empty() {
            return false;
        }

        let next_idx = match self.input_history_index {
            Some(0) => 0,
            Some(idx) => idx.saturating_sub(1),
            None => {
                self.input_history_draft = Some(self.input_text());
                self.input_history.len().saturating_sub(1)
            }
        };

        self.input_history_index = Some(next_idx);
        let text = self.input_history[next_idx].clone();
        self.set_input_text(&text);
        true
    }

    fn navigate_input_history_down(&mut self) -> bool {
        let Some(idx) = self.input_history_index else {
            return false;
        };

        if idx + 1 < self.input_history.len() {
            let next_idx = idx + 1;
            self.input_history_index = Some(next_idx);
            let text = self.input_history[next_idx].clone();
            self.set_input_text(&text);
            return true;
        }

        let draft = self.input_history_draft.take().unwrap_or_default();
        self.input_history_index = None;
        self.set_input_text(&draft);
        true
    }

    fn rewind_selected_message_idx(&self) -> Option<usize> {
        let idx = self.input_history_index?;
        self.input_history_message_idx.get(idx).and_then(|v| *v)
    }

    fn align_rewind_scroll_to_selected_prompt(&mut self, size: TerminalSize) {
        if !self.rewind_mode {
            return;
        }
        let Some(msg_idx) = self.rewind_selected_message_idx() else {
            return;
        };
        if self.messages.is_empty() {
            return;
        }
        let width = transcript_content_width(size);
        if width == 0 {
            return;
        }
        let upto = msg_idx.min(self.messages.len().saturating_sub(1));
        let rendered_upto =
            build_rendered_lines_with_hidden(&self.messages[..=upto], width, Some(msg_idx));
        if rendered_upto.is_empty() {
            return;
        }
        let input_layout = compute_input_layout(self, size);
        let msg_height = if input_layout.msg_bottom >= MSG_TOP {
            input_layout.msg_bottom - MSG_TOP + 1
        } else {
            0
        };
        if msg_height == 0 {
            return;
        }
        let target_line = rendered_upto.len().saturating_sub(1);
        self.scroll_top = target_line.saturating_sub(msg_height.saturating_sub(1));
    }

    fn input_apply_key(&mut self, key: crossterm::event::KeyEvent) {
        self.reset_esc_chord();
        self.reset_input_history_navigation();
        let _ = self.input.input(textarea_input_from_key(key));
    }

    fn input_insert_text(&mut self, text: String) {
        self.reset_esc_chord();
        self.reset_input_history_navigation();
        let _ = self.input.insert_str(text);
    }

    fn mark_transcript_dirty(&mut self) {
        self.transcript_dirty = true;
    }

    fn ensure_rendered_lines(&mut self, width: usize, hidden_user_message_idx: Option<usize>) {
        if self.transcript_dirty
            || self.rendered_width != width
            || self.rendered_hidden_user_message_idx != hidden_user_message_idx
        {
            self.rendered_lines =
                build_rendered_lines_with_hidden(&self.messages, width, hidden_user_message_idx);
            self.rendered_width = width;
            self.rendered_hidden_user_message_idx = hidden_user_message_idx;
            self.transcript_dirty = false;
        }
    }

    fn append_message(&mut self, role: Role, text: impl Into<String>) -> usize {
        self.messages.push(Message {
            role,
            text: text.into(),
            kind: MessageKind::Plain,
            file_path: None,
        });
        self.mark_transcript_dirty();
        self.messages.len() - 1
    }

    fn append_diff_message(
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
        self.mark_transcript_dirty();
        self.messages.len() - 1
    }

    fn put_agent_item_mapping(&mut self, item_id: &str, idx: usize) {
        self.agent_item_to_index.insert(item_id.to_string(), idx);
    }

    fn upsert_agent_delta(&mut self, item_id: &str, delta: &str) {
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
                self.mark_transcript_dirty();
            }
            return;
        }

        let idx = self.append_message(Role::Assistant, delta);
        self.put_agent_item_mapping(item_id, idx);
    }

    fn upsert_turn_diff(&mut self, turn_id: &str, diff: &str) {
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
                self.auto_follow_bottom = true;
                self.mark_transcript_dirty();
                return;
            }
        }

        let idx = self.append_diff_message(Role::ToolOutput, None, diff.to_string());
        self.turn_diff_to_index.insert(turn_id.to_string(), idx);
        self.auto_follow_bottom = true;
    }

    fn set_command_override(&mut self, call_id: &str, summary: String) {
        self.command_render_overrides
            .insert(call_id.to_string(), summary.clone());
        let mut changed = false;
        if let Some(idx) = self.agent_item_to_index.get(call_id).copied() {
            if let Some(msg) = self.messages.get_mut(idx) {
                msg.role = Role::ToolCall;
                msg.kind = MessageKind::Plain;
                msg.file_path = None;
                msg.text = summary;
                changed = true;
            }
        }
        if changed {
            self.mark_transcript_dirty();
        }
        self.auto_follow_bottom = true;
    }

    fn append_context_compacted_marker(&mut self) {
        const MARKER: &str = "↻ Context compacted";
        if let Some(last) = self.messages.last() {
            if last.role == Role::System && last.text == MARKER {
                return;
            }
        }
        self.append_message(Role::System, MARKER);
        self.auto_follow_bottom = true;
    }

    fn append_turn_interrupted_marker(&mut self) {
        const MARKER: &str = "Turn interrupted";
        if let Some(last) = self.messages.last() {
            if last.role == Role::System && last.text == MARKER {
                return;
            }
        }
        self.append_message(Role::System, MARKER);
        self.auto_follow_bottom = true;
    }
}

fn make_input_area() -> TextArea<'static> {
    TextArea::default()
}

fn is_tool_call_type(kind: &str) -> bool {
    matches!(
        kind,
        "toolCall" | "tool_call" | "toolInvocation" | "functionCall" | "mcpToolCall"
    )
}

fn is_tool_output_type(kind: &str) -> bool {
    matches!(
        kind,
        "toolResult" | "toolOutput" | "tool_result" | "functionCallOutput" | "mcpToolResult"
    )
}

fn role_for_tool_type(kind: &str) -> Option<Role> {
    if kind == "commandExecution" {
        return Some(Role::ToolCall);
    }
    if is_tool_call_type(kind) {
        return Some(Role::ToolCall);
    }
    if is_tool_output_type(kind) {
        return Some(Role::ToolOutput);
    }
    None
}

fn is_probably_diff_text(text: &str) -> bool {
    let t = text.trim();
    t.starts_with("diff --git ")
        || t.starts_with("@@ ")
        || (t.contains("\n@@ ") && (t.contains("\n+++ ") || t.contains("\n--- ")))
        || (t.contains('\n') && t.contains("\n+++ ") && t.contains("\n--- "))
}

fn infer_file_path_from_object(obj: &serde_json::Map<String, Value>) -> Option<String> {
    for key in ["filePath", "path", "file", "filename"] {
        if let Some(v) = obj.get(key).and_then(Value::as_str) {
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

fn collect_diff_blocks_recursive(
    value: &Value,
    current_file_path: Option<&str>,
    seen: &mut HashSet<String>,
    out: &mut Vec<DiffBlock>,
) {
    match value {
        Value::Object(obj) => {
            let inferred_path = infer_file_path_from_object(obj);
            let local_path = inferred_path.as_deref().or(current_file_path);

            if let Some(diff) = obj.get("diff").and_then(Value::as_str) {
                if !diff.is_empty() && is_probably_diff_text(diff) {
                    let key = format!("{}::{}", local_path.unwrap_or(""), diff);
                    if seen.insert(key) {
                        out.push(DiffBlock {
                            file_path: local_path.map(ToOwned::to_owned),
                            diff: diff.to_string(),
                        });
                    }
                }
            }

            for nested in obj.values() {
                collect_diff_blocks_recursive(nested, local_path, seen, out);
            }
        }
        Value::Array(arr) => {
            for nested in arr {
                collect_diff_blocks_recursive(nested, current_file_path, seen, out);
            }
        }
        _ => {}
    }
}

fn extract_diff_blocks(item: &Value) -> Vec<DiffBlock> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    collect_diff_blocks_recursive(item, None, &mut seen, &mut out);
    out
}

fn value_at_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut cur = value;
    for key in path {
        cur = cur.get(*key)?;
    }
    Some(cur)
}

fn first_string_at_paths(value: &Value, paths: &[&[&str]]) -> Option<String> {
    for path in paths {
        if let Some(s) = value_at_path(value, path).and_then(Value::as_str) {
            if !s.is_empty() {
                return Some(s.to_string());
            }
        }
    }
    None
}

fn first_i64_at_paths(value: &Value, paths: &[&[&str]]) -> Option<i64> {
    for path in paths {
        if let Some(v) = value_at_path(value, path) {
            if let Some(n) = v.as_i64() {
                return Some(n);
            }
            if let Some(n) = v.as_u64() {
                return i64::try_from(n).ok();
            }
        }
    }
    None
}

fn value_to_u64(v: &Value) -> Option<u64> {
    if let Some(n) = v.as_u64() {
        return Some(n);
    }
    if let Some(n) = v.as_i64() {
        return (n >= 0).then_some(n as u64);
    }
    None
}

fn select_context_used_tokens(total: Option<u64>, last: Option<u64>, max: u64) -> Option<u64> {
    match (total, last) {
        (Some(t), Some(l)) if t > max => Some(l),
        (Some(t), _) => Some(t),
        (None, Some(l)) => Some(l),
        (None, None) => None,
    }
}

fn context_usage_from_thread_token_usage_params(
    params: &serde_json::Map<String, Value>,
) -> Option<ContextUsage> {
    let token_usage = params.get("tokenUsage")?.as_object()?;
    let max = token_usage
        .get("modelContextWindow")
        .and_then(value_to_u64)
        .filter(|n| *n > 0)?;

    let total_used = token_usage
        .get("total")
        .and_then(Value::as_object)
        .and_then(|t| t.get("totalTokens"))
        .and_then(value_to_u64);
    let last_used = token_usage
        .get("last")
        .and_then(Value::as_object)
        .and_then(|t| t.get("totalTokens"))
        .and_then(value_to_u64);
    let used = select_context_used_tokens(total_used, last_used, max)?;

    Some(ContextUsage {
        used: used.min(max),
        max,
    })
}

fn context_usage_from_token_count_params(
    params: &serde_json::Map<String, Value>,
) -> Option<ContextUsage> {
    let info = params
        .get("msg")
        .and_then(Value::as_object)
        .and_then(|m| m.get("info"))
        .or_else(|| params.get("info"))?;
    let info_obj = info.as_object()?;

    let max = info_obj
        .get("model_context_window")
        .or_else(|| info_obj.get("modelContextWindow"))
        .and_then(value_to_u64)
        .filter(|n| *n > 0)?;

    let total_used = info_obj
        .get("total_token_usage")
        .and_then(Value::as_object)
        .and_then(|t| t.get("total_tokens"))
        .and_then(value_to_u64)
        .or_else(|| {
            info_obj
                .get("total")
                .and_then(Value::as_object)
                .and_then(|t| t.get("totalTokens").or_else(|| t.get("total_tokens")))
                .and_then(value_to_u64)
        });
    let last_used = info_obj
        .get("last_token_usage")
        .and_then(Value::as_object)
        .and_then(|t| t.get("total_tokens"))
        .and_then(value_to_u64)
        .or_else(|| {
            info_obj
                .get("last")
                .and_then(Value::as_object)
                .and_then(|t| t.get("totalTokens").or_else(|| t.get("total_tokens")))
                .and_then(value_to_u64)
        });
    let used = select_context_used_tokens(total_used, last_used, max)?;

    Some(ContextUsage {
        used: used.min(max),
        max,
    })
}

fn context_usage_compact_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{}m", n / 1_000_000)
    } else if n >= 1_000 {
        format!("{}k", n / 1_000)
    } else {
        n.to_string()
    }
}

fn context_usage_label(usage: ContextUsage) -> String {
    if usage.max == 0 {
        return String::new();
    }
    let pct = ((usage.used as f64 / usage.max as f64) * 100.0).round() as u64;
    format!(
        "{}/{} ({}%)",
        context_usage_compact_tokens(usage.used),
        context_usage_compact_tokens(usage.max),
        pct.min(100)
    )
}

fn context_usage_placeholder_label() -> &'static str {
    "___k/___k (__%)"
}

fn context_label_reserved_cells(context_label: Option<&str>) -> usize {
    let base = visual_width("999k/999k (99%)").max(visual_width(context_usage_placeholder_label()));
    let label_cells = context_label.map(visual_width).unwrap_or(0);
    base.max(label_cells)
}

fn command_execution_action_command(item: &Value) -> Option<String> {
    let actions = item.get("commandActions").and_then(Value::as_array)?;
    for a in actions {
        if let Some(cmd) = a.get("command").and_then(Value::as_str) {
            if !cmd.is_empty() {
                return Some(cmd.to_string());
            }
        }
    }
    None
}

fn normalize_shell_command(raw: &str) -> String {
    if let Some(pos) = raw.find(" -lc '") {
        let s = &raw[(pos + 6)..];
        if let Some(end) = s.rfind('\'') {
            return s[..end].to_string();
        }
    }
    raw.to_string()
}

fn tool_name(item: &Value) -> Option<String> {
    first_string_at_paths(
        item,
        &[
            &["tool"],
            &["name"],
            &["input", "tool"],
            &["input", "name"],
            &["function", "name"],
        ],
    )
}

fn tool_command(item: &Value) -> Option<String> {
    if item.get("type").and_then(Value::as_str) == Some("commandExecution") {
        if let Some(cmd) = command_execution_action_command(item) {
            return Some(cmd);
        }
        if let Some(raw) = item.get("command").and_then(Value::as_str) {
            if !raw.is_empty() {
                return Some(normalize_shell_command(raw));
            }
        }
    }

    first_string_at_paths(
        item,
        &[
            &["command"],
            &["input", "command"],
            &["action", "command"],
            &["args", "command"],
            &["arguments", "command"],
            &["metadata", "command"],
        ],
    )
}

fn tool_reasoning(item: &Value) -> Option<String> {
    first_string_at_paths(
        item,
        &[
            &["reasoning"],
            &["input", "reasoning"],
            &["metadata", "reasoning"],
            &["thought"],
        ],
    )
}

fn tool_description(item: &Value) -> Option<String> {
    first_string_at_paths(
        item,
        &[
            &["description"],
            &["input", "description"],
            &["metadata", "description"],
            &["title"],
            &["metadata", "title"],
        ],
    )
}

fn tool_output_text(item: &Value) -> Option<String> {
    let mut parts = Vec::new();

    if let Some(s) = first_string_at_paths(item, &[&["aggregatedOutput"]]) {
        parts.push(s);
    }

    if let Some(s) = first_string_at_paths(
        item,
        &[
            &["output"],
            &["text"],
            &["result"],
            &["metadata", "output"],
            &["metadata", "result"],
            &["state", "output"],
            &["formattedOutput"],
        ],
    ) {
        parts.push(s);
    }
    if let Some(s) = first_string_at_paths(
        item,
        &[&["stdout"], &["metadata", "stdout"], &["state", "stdout"]],
    ) {
        parts.push(s);
    }
    if let Some(s) = first_string_at_paths(
        item,
        &[&["stderr"], &["metadata", "stderr"], &["state", "stderr"]],
    ) {
        parts.push(s);
    }
    if let Some(code) = first_i64_at_paths(
        item,
        &[
            &["exitCode"],
            &["exit_code"],
            &["metadata", "exitCode"],
            &["metadata", "exit_code"],
            &["state", "exitCode"],
            &["durationMs"],
        ],
    ) {
        if item.get("durationMs").is_some() && item.get("exitCode").is_none() {
            parts.push(format!("duration: {code} ms"));
        } else {
            parts.push(format!("exit code: {code}"));
        }
    }

    if let Some(code) = first_i64_at_paths(item, &[&["exitCode"], &["exit_code"]]) {
        if !parts.iter().any(|p| p.starts_with("exit code:")) {
            parts.push(format!("exit code: {code}"));
        }
    }

    parts.retain(|s| !s.trim().is_empty());
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

fn command_execution_diff_output(item: &Value) -> Option<String> {
    if item.get("type").and_then(Value::as_str) != Some("commandExecution") {
        return None;
    }
    let candidates = [
        item.get("aggregatedOutput").and_then(Value::as_str),
        item.get("formattedOutput").and_then(Value::as_str),
        item.get("stdout").and_then(Value::as_str),
        item.get("output").and_then(Value::as_str),
    ];
    for c in candidates.into_iter().flatten() {
        if !c.trim().is_empty() && is_probably_diff_text(c) {
            return Some(c.to_string());
        }
    }
    None
}

fn compact_command_path(path: &str, cwd: Option<&str>) -> String {
    if let Some(cwd) = cwd {
        if let Some(rest) = path.strip_prefix(cwd) {
            if let Some(trimmed) = rest.strip_prefix('/') {
                if !trimmed.is_empty() {
                    return trimmed.to_string();
                }
            }
        }
    }
    path.to_string()
}

fn first_non_option_token(command: &str) -> Option<&str> {
    command
        .split_whitespace()
        .find(|t| !t.is_empty() && !t.starts_with('-'))
}

fn strip_shell_quotes(token: &str) -> &str {
    let t = token.trim();
    if t.len() >= 2 {
        if (t.starts_with('\'') && t.ends_with('\'')) || (t.starts_with('"') && t.ends_with('"')) {
            return &t[1..(t.len() - 1)];
        }
    }
    t
}

fn command_summary_from_shell_cmd(cmd: &str, cwd: Option<&str>) -> Option<String> {
    let cmd = normalize_shell_command(cmd);
    let cmd = cmd.trim();
    if cmd.is_empty() {
        return None;
    }

    let lower = cmd.to_ascii_lowercase();
    if lower.starts_with("git diff ") || lower == "git diff" || lower.starts_with("diff ") {
        return Some("Δ Diff".to_string());
    }
    if lower.starts_with("apply_patch") || lower.starts_with("patch ") {
        return Some("← Edit".to_string());
    }
    if lower.starts_with("rg ")
        || lower.starts_with("grep ")
        || lower.starts_with("fd ")
        || lower.starts_with("find ")
        || lower.starts_with("git grep ")
    {
        return Some("✱ Search".to_string());
    }

    let lhs = cmd.split('|').next().unwrap_or(cmd).trim();
    let lhs_lower = lhs.to_ascii_lowercase();

    if lhs_lower.starts_with("nl ")
        || lhs_lower.starts_with("cat ")
        || lhs_lower.starts_with("bat ")
        || lhs_lower.starts_with("head ")
        || lhs_lower.starts_with("tail ")
    {
        let sub = lhs
            .split_once(' ')
            .map(|(_, rest)| rest)
            .unwrap_or("")
            .trim();
        if let Some(path_token) = first_non_option_token(sub) {
            let path = strip_shell_quotes(path_token);
            if !path.is_empty() {
                return Some(format!("→ Read {}", compact_command_path(path, cwd)));
            }
        }
        return Some("→ Read".to_string());
    }

    None
}

fn command_summary_from_parsed_cmd(msg: &Value) -> Option<(String, String)> {
    let call_id = msg.get("call_id").and_then(Value::as_str)?.to_string();
    let parsed = msg.get("parsed_cmd").and_then(Value::as_array)?;
    let cwd = msg.get("cwd").and_then(Value::as_str);

    for part in parsed {
        let Some(obj) = part.as_object() else {
            continue;
        };
        let kind = obj
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_ascii_lowercase();
        let cmd = obj
            .get("cmd")
            .and_then(Value::as_str)
            .or_else(|| obj.get("command").and_then(Value::as_str));

        let path = obj
            .get("path")
            .and_then(Value::as_str)
            .or_else(|| obj.get("filePath").and_then(Value::as_str))
            .or_else(|| obj.get("file").and_then(Value::as_str))
            .or_else(|| obj.get("name").and_then(Value::as_str))
            .unwrap_or("")
            .trim();

        let path_display = if path.is_empty() {
            None
        } else {
            Some(compact_command_path(path, cwd))
        };

        let mut out = if kind == "read" {
            "→ Read".to_string()
        } else if matches!(
            kind.as_str(),
            "grep" | "rg" | "search" | "glob" | "find" | "codesearch"
        ) {
            "✱ Search".to_string()
        } else if matches!(kind.as_str(), "list" | "listfiles" | "list_files" | "ls") {
            "→ List".to_string()
        } else if matches!(
            kind.as_str(),
            "write" | "edit" | "applypatch" | "apply_patch" | "replace"
        ) {
            "← Edit".to_string()
        } else if matches!(kind.as_str(), "diff" | "gitdiff" | "git_diff") {
            "Δ Diff".to_string()
        } else if let Some(cmd) = cmd {
            let Some(summary) = command_summary_from_shell_cmd(cmd, cwd) else {
                continue;
            };
            summary
        } else {
            continue;
        };

        if out == "→ Read"
            || out == "✱ Search"
            || out == "→ List"
            || out == "← Edit"
            || out == "Δ Diff"
        {
            if let Some(display) = path_display {
                out.push(' ');
                out.push_str(&display);
            }
        }

        if let Some(args) = format_input_brackets(
            obj,
            &[
                "type", "cmd", "command", "path", "filePath", "file", "name", "cwd",
            ],
        ) {
            out.push(' ');
            out.push_str(&args);
        }

        return Some((call_id, out));
    }

    None
}

fn compact_json_summary(value: &Value, max_chars: usize) -> Option<String> {
    let mut s = serde_json::to_string(value).ok()?;
    if s.len() > max_chars {
        s.truncate(max_chars.saturating_sub(1));
        s.push('…');
    }
    Some(s)
}

fn tool_input_object<'a>(item: &'a Value) -> Option<&'a serde_json::Map<String, Value>> {
    value_at_path(item, &["input"]).and_then(Value::as_object)
}

fn inline_value(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => {
            if s.is_empty() {
                None
            } else {
                Some(s.clone())
            }
        }
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null => None,
        Value::Array(_) | Value::Object(_) => compact_json_summary(value, 64),
    }
}

fn format_input_brackets(
    input: &serde_json::Map<String, Value>,
    skip_keys: &[&str],
) -> Option<String> {
    let skip: HashSet<&str> = skip_keys.iter().copied().collect();
    let mut keys: Vec<&str> = input
        .keys()
        .map(String::as_str)
        .filter(|k| !skip.contains(*k))
        .collect();
    keys.sort_unstable();

    let mut parts = Vec::new();
    for key in keys {
        let Some(val) = input.get(key).and_then(inline_value) else {
            continue;
        };
        if val.is_empty() {
            continue;
        }
        parts.push(format!("{key}={val}"));
    }

    if parts.is_empty() {
        None
    } else {
        Some(format!("[{}]", parts.join(" ")))
    }
}

fn titlecase_tool_name(name: &str) -> String {
    let normalized = name.replace(['_', '-'], " ");
    let mut out = String::new();
    for (i, word) in normalized.split_whitespace().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            out.push_str(chars.as_str());
        }
    }
    if out.is_empty() {
        name.to_string()
    } else {
        out
    }
}

fn tool_icon(tool_name: &str) -> &'static str {
    match tool_name {
        "read" | "list" => "→",
        "write" | "edit" | "applypatch" | "apply_patch" => "←",
        "grep" | "glob" | "codesearch" => "✱",
        "task" => "#",
        _ => "◇",
    }
}

fn format_tool_call_inline(item: &Value, tool_name: &str) -> Option<String> {
    let lower = tool_name.to_ascii_lowercase();
    let input = tool_input_object(item);

    match lower.as_str() {
        "read" => {
            let path = input
                .and_then(|obj| {
                    obj.get("filePath")
                        .and_then(Value::as_str)
                        .or_else(|| obj.get("path").and_then(Value::as_str))
                })
                .unwrap_or("");
            let mut out = format!("{} Read {}", tool_icon(&lower), path);
            if let Some(args) = input.and_then(|obj| {
                format_input_brackets(
                    obj,
                    &[
                        "filePath",
                        "path",
                        "tool",
                        "name",
                        "command",
                        "description",
                        "reasoning",
                    ],
                )
            }) {
                if !args.is_empty() {
                    out.push(' ');
                    out.push_str(&args);
                }
            }
            return Some(out.trim_end().to_string());
        }
        _ => {}
    }

    if let Some(obj) = input {
        let mut out = format!("{} {}", tool_icon(&lower), titlecase_tool_name(tool_name));
        if let Some(path) = obj
            .get("filePath")
            .and_then(Value::as_str)
            .or_else(|| obj.get("path").and_then(Value::as_str))
        {
            if !path.is_empty() {
                out.push(' ');
                out.push_str(path);
            }
        }

        if let Some(args) = format_input_brackets(
            obj,
            &[
                "filePath",
                "path",
                "tool",
                "name",
                "command",
                "description",
                "reasoning",
                "content",
                "patch",
                "old_string",
                "new_string",
                "text",
            ],
        ) {
            out.push(' ');
            out.push_str(&args);
        }
        return Some(out);
    }

    Some(format!(
        "{} {}",
        tool_icon(&lower),
        titlecase_tool_name(tool_name)
    ))
}

fn tool_input_summary(item: &Value) -> Option<String> {
    if let Some(obj) = value_at_path(item, &["input"]).and_then(Value::as_object) {
        let mut fields = Vec::new();
        for key in [
            "filePath",
            "path",
            "pattern",
            "query",
            "url",
            "method",
            "subagent_type",
        ] {
            if let Some(v) = obj.get(key).and_then(Value::as_str) {
                if !v.is_empty() {
                    fields.push(format!("{key}={v}"));
                }
            }
        }
        if !fields.is_empty() {
            return Some(fields.join(" "));
        }
        return compact_json_summary(&Value::Object(obj.clone()), 220);
    }
    None
}

fn format_tool_item(item: &Value, role: Role) -> Option<String> {
    match role {
        Role::ToolCall => {
            if let Some(cmd) = tool_command(item) {
                let mut lines = Vec::new();
                lines.push(format!("run `{cmd}`"));
                if let Some(reason) = tool_reasoning(item) {
                    lines.push(format!("Thinking: {reason}"));
                }
                if let Some(desc) = tool_description(item) {
                    lines.push(format!("# {desc}"));
                }
                lines.push(format!("$ {cmd}"));
                return Some(lines.join("\n"));
            }

            if let Some(name) = tool_name(item) {
                if let Some(inline) = format_tool_call_inline(item, &name) {
                    return Some(inline);
                }
                if let Some(input) = tool_input_summary(item) {
                    return Some(format!("{} {}", titlecase_tool_name(&name), input));
                }
                return Some(titlecase_tool_name(&name));
            }

            None
        }
        Role::ToolOutput => {
            if item.get("type").and_then(Value::as_str) == Some("commandExecution") {
                let command = tool_command(item);
                let output = tool_output_text(item);

                return match (command, output) {
                    (Some(cmd), Some(out)) => Some(format!("$ {cmd}\n{out}")),
                    (Some(cmd), None) => Some(format!("$ {cmd}")),
                    (None, Some(out)) => Some(out),
                    (None, None) => None,
                };
            }

            if let Some(out) = tool_output_text(item) {
                return Some(out);
            }
            None
        }
        _ => None,
    }
}

fn collect_text_parts(content: &[Value]) -> Vec<&str> {
    let mut text_parts = Vec::new();
    for part in content {
        if part.get("type").and_then(Value::as_str) != Some("text") {
            continue;
        }
        if let Some(t) = part.get("text").and_then(Value::as_str) {
            text_parts.push(t);
        }
    }
    text_parts
}

fn item_text_from_content(item: &Value) -> Option<String> {
    let content = item.get("content").and_then(Value::as_array)?;
    let text_parts = collect_text_parts(content);
    if text_parts.is_empty() {
        None
    } else {
        Some(text_parts.join("\n"))
    }
}

fn append_item_text_from_content(app: &mut AppState, item: &Value, role: Role) {
    if let Some(text) = item_text_from_content(item) {
        app.append_message(role, text);
    }
}

fn append_tool_history_item(app: &mut AppState, item: &Value, role: Role) {
    let diffs = extract_diff_blocks(item);
    if !diffs.is_empty() {
        for block in diffs {
            app.append_diff_message(role, block.file_path, block.diff);
        }
        return;
    }

    if role == Role::ToolOutput {
        if let Some(diff) = command_execution_diff_output(item) {
            app.append_diff_message(role, None, diff);
            return;
        }
    }

    if let Some(formatted) = format_tool_item(item, role) {
        if !formatted.is_empty() {
            app.append_message(role, formatted);
            return;
        }
    }

    if let Some(t) = item.get("text").and_then(Value::as_str) {
        if !t.is_empty() {
            app.append_message(role, t.to_string());
            return;
        }
    }
    append_item_text_from_content(app, item, role);
}

fn parse_arguments_value(arguments: &Value) -> Option<Value> {
    match arguments {
        Value::Null => None,
        Value::String(s) => {
            if s.trim().is_empty() {
                None
            } else {
                serde_json::from_str::<Value>(s)
                    .ok()
                    .or_else(|| Some(Value::String(s.clone())))
            }
        }
        other => Some(other.clone()),
    }
}

fn raw_function_call_to_tool_item(item: &Value) -> Option<(String, Value)> {
    if item.get("type").and_then(Value::as_str) != Some("function_call") {
        return None;
    }
    let call_id = item
        .get("call_id")
        .and_then(Value::as_str)
        .or_else(|| item.get("id").and_then(Value::as_str))?
        .to_string();
    let name = item.get("name").and_then(Value::as_str).unwrap_or("");

    let mut out = serde_json::Map::new();
    out.insert(
        "type".to_string(),
        Value::String(if name == "exec_command" {
            "commandExecution".to_string()
        } else {
            "toolCall".to_string()
        }),
    );
    if !name.is_empty() {
        out.insert("tool".to_string(), Value::String(name.to_string()));
        out.insert("name".to_string(), Value::String(name.to_string()));
    }

    if let Some(args) = item.get("arguments").and_then(parse_arguments_value) {
        if let Value::Object(obj) = &args {
            out.insert("input".to_string(), Value::Object(obj.clone()));
            if name == "exec_command" {
                if let Some(cmd) = obj.get("cmd").and_then(Value::as_str) {
                    out.insert("command".to_string(), Value::String(cmd.to_string()));
                    out.insert(
                        "commandActions".to_string(),
                        json!([{ "type": "unknown", "command": cmd }]),
                    );
                }
            }
        } else {
            out.insert("input".to_string(), json!({ "value": args }));
        }
    }

    Some((call_id, Value::Object(out)))
}

fn raw_function_call_output_to_tool_item(item: &Value) -> Option<(String, Value)> {
    if item.get("type").and_then(Value::as_str) != Some("function_call_output") {
        return None;
    }
    let call_id = item
        .get("call_id")
        .and_then(Value::as_str)
        .or_else(|| item.get("id").and_then(Value::as_str))?
        .to_string();

    let output_value = item.get("output").cloned().unwrap_or(Value::Null);
    let mut out = serde_json::Map::new();
    out.insert("type".to_string(), Value::String("toolOutput".to_string()));
    match output_value {
        Value::String(s) => {
            out.insert("output".to_string(), Value::String(s.clone()));
            if is_probably_diff_text(&s) {
                out.insert("diff".to_string(), Value::String(s));
            }
        }
        Value::Object(obj) => {
            for (k, v) in obj {
                out.insert(k, v);
            }
        }
        Value::Null => {}
        other => {
            out.insert("output".to_string(), other);
        }
    }

    Some((call_id, Value::Object(out)))
}

fn upsert_tool_message(
    app: &mut AppState,
    key: &str,
    role: Role,
    text: String,
    kind: MessageKind,
    file_path: Option<String>,
) {
    if let Some(idx) = app.agent_item_to_index.get(key).copied() {
        if let Some(msg) = app.messages.get_mut(idx) {
            msg.role = role;
            msg.text = text;
            msg.kind = kind;
            msg.file_path = file_path;
            app.auto_follow_bottom = true;
            app.mark_transcript_dirty();
            return;
        }
    }

    let idx = if kind == MessageKind::Diff {
        app.append_diff_message(role, file_path, text)
    } else {
        app.append_message(role, text)
    };
    app.put_agent_item_mapping(key, idx);
    app.auto_follow_bottom = true;
}

fn handle_raw_response_item(app: &mut AppState, item: &Value) {
    if let Some((call_id, tool_item)) = raw_function_call_to_tool_item(item) {
        if app.agent_item_to_index.contains_key(&call_id) {
            return;
        }
        if let Some(formatted) = format_tool_item(&tool_item, Role::ToolCall) {
            if !formatted.trim().is_empty() {
                upsert_tool_message(
                    app,
                    &call_id,
                    Role::ToolCall,
                    formatted,
                    MessageKind::Plain,
                    None,
                );
            }
        }
        return;
    }

    if let Some((call_id, tool_item)) = raw_function_call_output_to_tool_item(item) {
        let diffs = extract_diff_blocks(&tool_item);
        if let Some(first) = diffs.first() {
            upsert_tool_message(
                app,
                &call_id,
                Role::ToolOutput,
                first.diff.clone(),
                MessageKind::Diff,
                first.file_path.clone(),
            );
            for block in diffs.iter().skip(1) {
                app.append_diff_message(
                    Role::ToolOutput,
                    block.file_path.clone(),
                    block.diff.clone(),
                );
            }
            app.auto_follow_bottom = true;
            return;
        }

        if let Some(formatted) = format_tool_item(&tool_item, Role::ToolOutput) {
            if !formatted.trim().is_empty() {
                upsert_tool_message(
                    app,
                    &call_id,
                    Role::ToolOutput,
                    formatted,
                    MessageKind::Plain,
                    None,
                );
            }
        }
    }
}

fn append_history_from_thread(app: &mut AppState, thread_obj: &Value) {
    let Some(turns) = thread_obj.get("turns").and_then(Value::as_array) else {
        return;
    };

    for turn in turns {
        let Some(items) = turn.get("items").and_then(Value::as_array) else {
            continue;
        };
        for item in items {
            let Some(kind) = item.get("type").and_then(Value::as_str) else {
                continue;
            };

            match kind {
                "userMessage" => {
                    if let Some(text) = item_text_from_content(item) {
                        let idx = app.append_message(Role::User, text.clone());
                        app.record_input_history(&text, Some(idx));
                    }
                }
                "agentMessage" => {
                    if let Some(t) = item.get("text").and_then(Value::as_str) {
                        app.append_message(Role::Assistant, t.to_string());
                    }
                }
                "reasoning" => {
                    let Some(summary) = item.get("summary").and_then(Value::as_array) else {
                        continue;
                    };
                    let mut parts = Vec::new();
                    for s in summary {
                        if let Some(t) = s.as_str() {
                            parts.push(t);
                        }
                    }
                    if !parts.is_empty() {
                        app.append_message(Role::Reasoning, parts.join("\n"));
                    }
                }
                "commandExecution" => {
                    append_tool_history_item(app, item, Role::ToolOutput);
                }
                "contextCompaction" => {
                    app.append_context_compacted_marker();
                }
                k if is_tool_call_type(k) => {
                    append_tool_history_item(app, item, Role::ToolCall);
                }
                k if is_tool_output_type(k) => {
                    append_tool_history_item(app, item, Role::ToolOutput);
                }
                _ => {}
            }
        }
    }
}

fn load_history_from_start_or_resume(app: &mut AppState, response_line: &str) -> Result<()> {
    let parsed = extract_result_object(response_line)?;
    if let Some(thread_obj) = parsed
        .get("result")
        .and_then(Value::as_object)
        .and_then(|r| r.get("thread"))
    {
        append_history_from_thread(app, thread_obj);
    }
    Ok(())
}

fn parse_thread_list(response_line: &str) -> Result<Vec<ThreadSummary>> {
    let parsed = extract_result_object(response_line)?;
    let Some(data) = parsed
        .get("result")
        .and_then(Value::as_object)
        .and_then(|r| r.get("data"))
        .and_then(Value::as_array)
    else {
        return Ok(Vec::new());
    };

    let mut out = Vec::new();
    for entry in data {
        let Some(obj) = entry.as_object() else {
            continue;
        };
        let Some(id) = obj.get("id").and_then(Value::as_str) else {
            continue;
        };
        let preview = obj.get("preview").and_then(Value::as_str).unwrap_or("");
        let cwd = obj.get("cwd").and_then(Value::as_str).unwrap_or("");
        let updated_at = obj
            .get("updatedAt")
            .and_then(|v| v.as_i64().or_else(|| v.as_u64().map(|n| n as i64)))
            .unwrap_or(0);

        out.push(ThreadSummary {
            id: id.to_string(),
            preview: preview.to_string(),
            cwd: cwd.to_string(),
            updated_at,
        });
    }
    Ok(out)
}

fn role_prefix(role: Role) -> &'static str {
    match role {
        Role::User => "",
        Role::Assistant => "",
        Role::Reasoning => "",
        Role::ToolCall => "",
        Role::ToolOutput => "",
        Role::System => "",
    }
}

#[derive(Debug, Clone, Copy)]
struct CarlosMarkdownStyleSheet;

fn color_to_core(color: Color) -> CoreColor {
    match color {
        Color::Reset => CoreColor::Reset,
        Color::Black => CoreColor::Black,
        Color::Red => CoreColor::Red,
        Color::Green => CoreColor::Green,
        Color::Yellow => CoreColor::Yellow,
        Color::Blue => CoreColor::Blue,
        Color::Magenta => CoreColor::Magenta,
        Color::Cyan => CoreColor::Cyan,
        Color::Gray => CoreColor::Gray,
        Color::DarkGray => CoreColor::DarkGray,
        Color::LightRed => CoreColor::LightRed,
        Color::LightGreen => CoreColor::LightGreen,
        Color::LightYellow => CoreColor::LightYellow,
        Color::LightBlue => CoreColor::LightBlue,
        Color::LightMagenta => CoreColor::LightMagenta,
        Color::LightCyan => CoreColor::LightCyan,
        Color::White => CoreColor::White,
        Color::Rgb(r, g, b) => CoreColor::Rgb(r, g, b),
        Color::Indexed(v) => CoreColor::Indexed(v),
    }
}

fn core_color_to_color(color: CoreColor) -> Color {
    match color {
        CoreColor::Reset => Color::Reset,
        CoreColor::Black => Color::Black,
        CoreColor::Red => Color::Red,
        CoreColor::Green => Color::Green,
        CoreColor::Yellow => Color::Yellow,
        CoreColor::Blue => Color::Blue,
        CoreColor::Magenta => Color::Magenta,
        CoreColor::Cyan => Color::Cyan,
        CoreColor::Gray => Color::Gray,
        CoreColor::DarkGray => Color::DarkGray,
        CoreColor::LightRed => Color::LightRed,
        CoreColor::LightGreen => Color::LightGreen,
        CoreColor::LightYellow => Color::LightYellow,
        CoreColor::LightBlue => Color::LightBlue,
        CoreColor::LightMagenta => Color::LightMagenta,
        CoreColor::LightCyan => Color::LightCyan,
        CoreColor::White => Color::White,
        CoreColor::Rgb(r, g, b) => Color::Rgb(r, g, b),
        CoreColor::Indexed(v) => Color::Indexed(v),
    }
}

fn modifier_to_core(modifier: Modifier) -> CoreModifier {
    let mut out = CoreModifier::empty();
    if modifier.contains(Modifier::BOLD) {
        out |= CoreModifier::BOLD;
    }
    if modifier.contains(Modifier::DIM) {
        out |= CoreModifier::DIM;
    }
    if modifier.contains(Modifier::ITALIC) {
        out |= CoreModifier::ITALIC;
    }
    if modifier.contains(Modifier::UNDERLINED) {
        out |= CoreModifier::UNDERLINED;
    }
    if modifier.contains(Modifier::SLOW_BLINK) {
        out |= CoreModifier::SLOW_BLINK;
    }
    if modifier.contains(Modifier::RAPID_BLINK) {
        out |= CoreModifier::RAPID_BLINK;
    }
    if modifier.contains(Modifier::REVERSED) {
        out |= CoreModifier::REVERSED;
    }
    if modifier.contains(Modifier::HIDDEN) {
        out |= CoreModifier::HIDDEN;
    }
    if modifier.contains(Modifier::CROSSED_OUT) {
        out |= CoreModifier::CROSSED_OUT;
    }
    out
}

fn core_modifier_to_modifier(modifier: CoreModifier) -> Modifier {
    let mut out = Modifier::empty();
    if modifier.contains(CoreModifier::BOLD) {
        out |= Modifier::BOLD;
    }
    if modifier.contains(CoreModifier::DIM) {
        out |= Modifier::DIM;
    }
    if modifier.contains(CoreModifier::ITALIC) {
        out |= Modifier::ITALIC;
    }
    if modifier.contains(CoreModifier::UNDERLINED) {
        out |= Modifier::UNDERLINED;
    }
    if modifier.contains(CoreModifier::SLOW_BLINK) {
        out |= Modifier::SLOW_BLINK;
    }
    if modifier.contains(CoreModifier::RAPID_BLINK) {
        out |= Modifier::RAPID_BLINK;
    }
    if modifier.contains(CoreModifier::REVERSED) {
        out |= Modifier::REVERSED;
    }
    if modifier.contains(CoreModifier::HIDDEN) {
        out |= Modifier::HIDDEN;
    }
    if modifier.contains(CoreModifier::CROSSED_OUT) {
        out |= Modifier::CROSSED_OUT;
    }
    out
}

fn style_to_core(style: Style) -> CoreStyle {
    let mut out = CoreStyle::default();
    if let Some(fg) = style.fg {
        out = out.fg(color_to_core(fg));
    }
    if let Some(bg) = style.bg {
        out = out.bg(color_to_core(bg));
    }
    if let Some(ul) = style.underline_color {
        out = out.underline_color(color_to_core(ul));
    }
    out.add_modifier = modifier_to_core(style.add_modifier);
    out.sub_modifier = modifier_to_core(style.sub_modifier);
    out
}

fn core_style_to_style(style: CoreStyle) -> Style {
    let mut out = Style::default();
    if let Some(fg) = style.fg {
        out = out.fg(core_color_to_color(fg));
    }
    if let Some(bg) = style.bg {
        out = out.bg(core_color_to_color(bg));
    }
    if let Some(ul) = style.underline_color {
        out = out.underline_color(core_color_to_color(ul));
    }
    out.add_modifier = core_modifier_to_modifier(style.add_modifier);
    out.sub_modifier = core_modifier_to_modifier(style.sub_modifier);
    out
}

impl tui_markdown::StyleSheet for CarlosMarkdownStyleSheet {
    fn heading(&self, level: u8) -> CoreStyle {
        match level {
            1 => style_to_core(
                Style::default()
                    .fg(COLOR_TEXT)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            ),
            2 => style_to_core(Style::default().fg(COLOR_TEXT).add_modifier(Modifier::BOLD)),
            _ => style_to_core(Style::default().fg(COLOR_TEXT)),
        }
    }

    fn code(&self) -> CoreStyle {
        style_to_core(Style::default().fg(COLOR_TEXT))
    }

    fn link(&self) -> CoreStyle {
        style_to_core(
            Style::default()
                .fg(COLOR_GUTTER_USER)
                .add_modifier(Modifier::UNDERLINED),
        )
    }

    fn blockquote(&self) -> CoreStyle {
        style_to_core(Style::default().fg(COLOR_DIM))
    }

    fn heading_meta(&self) -> CoreStyle {
        style_to_core(Style::default().fg(COLOR_DIM).add_modifier(Modifier::DIM))
    }

    fn metadata_block(&self) -> CoreStyle {
        style_to_core(Style::default().fg(COLOR_DIM))
    }
}

fn is_fence_delimiter(line: &str) -> bool {
    line.trim_matches([' ', '\t', '\r']).starts_with("```")
}

fn styled_plain_text(segments: &[StyledSegment]) -> String {
    let mut out = String::new();
    for seg in segments {
        out.push_str(&seg.text);
    }
    out
}

fn markdown_line_segments(text: &str) -> Vec<Vec<StyledSegment>> {
    let opts = MarkdownOptions::new(CarlosMarkdownStyleSheet);
    let markdown = markdown_from_str_with_options(text, &opts);

    let mut out = Vec::new();
    for line in markdown.lines {
        let mut segments = Vec::new();
        for span in line.spans {
            if span.content.is_empty() {
                continue;
            }
            segments.push(StyledSegment {
                text: span.content.to_string(),
                style: core_style_to_style(span.style),
            });
        }

        let plain = styled_plain_text(&segments);
        if is_fence_delimiter(&plain) {
            continue;
        }
        out.push(segments);
    }
    out
}

fn take_styled_segments_by_cells(
    remaining: &mut VecDeque<StyledSegment>,
    max_cells: usize,
) -> Vec<StyledSegment> {
    if max_cells == 0 {
        return Vec::new();
    }

    let mut out = Vec::new();
    let mut taken_cells = 0usize;

    while let Some(mut seg) = remaining.pop_front() {
        let seg_cells = visual_width(&seg.text);
        if seg_cells == 0 {
            out.push(seg);
            continue;
        }

        if taken_cells + seg_cells <= max_cells {
            taken_cells += seg_cells;
            out.push(seg);
            if taken_cells == max_cells {
                break;
            }
            continue;
        }

        let allowed = max_cells.saturating_sub(taken_cells);
        if allowed == 0 {
            remaining.push_front(seg);
            break;
        }

        let split = split_at_cells(&seg.text, allowed);
        if split == 0 {
            remaining.push_front(seg);
            break;
        }

        let left = seg.text[..split].to_string();
        let right = seg.text[split..].to_string();
        let seg_style = seg.style;
        if !right.is_empty() {
            seg.text = right;
            remaining.push_front(seg);
        }

        out.push(StyledSegment {
            text: left,
            style: seg_style,
        });
        break;
    }

    out
}

fn normalize_styled_segments_for_part(
    part: &str,
    styled_segments: Vec<StyledSegment>,
) -> Vec<StyledSegment> {
    if styled_plain_text(&styled_segments) == part {
        styled_segments
    } else {
        vec![StyledSegment {
            text: part.to_string(),
            style: Style::default(),
        }]
    }
}

fn append_wrapped_message_lines(out: &mut Vec<RenderedLine>, role: Role, text: &str, width: usize) {
    if width < 8 {
        return;
    }

    let prefix = role_prefix(role);
    let mut first_physical = true;
    let mut in_code_fence = false;

    for logical in text.split('\n') {
        if is_fence_delimiter(logical) {
            in_code_fence = !in_code_fence;
            continue;
        }

        let continuation = role_prefix(role);

        if logical.is_empty() {
            let t = prefix.to_string();
            out.push(RenderedLine {
                cells: visual_width(&t),
                text: t,
                styled_segments: Vec::new(),
                role,
                separator: false,
                soft_wrap_to_next: false,
            });
            first_physical = false;
            continue;
        }

        let lead_for_width = if in_code_fence {
            role_prefix(Role::System)
        } else if first_physical {
            prefix
        } else {
            continuation
        };
        let lead_cells = visual_width(lead_for_width);
        let avail = width.saturating_sub(lead_cells);
        if avail == 0 {
            first_physical = false;
            continue;
        }
        let wrapped_parts = wrap_natural_by_cells(logical, avail);

        for (i, part) in wrapped_parts.iter().enumerate() {
            let lead = if in_code_fence {
                role_prefix(Role::System)
            } else if first_physical && i == 0 {
                prefix
            } else {
                continuation
            };

            let mut line = String::with_capacity(lead.len() + part.len());
            line.push_str(lead);
            line.push_str(part);
            let wrapped = i + 1 < wrapped_parts.len();

            out.push(RenderedLine {
                cells: visual_width(&line),
                text: line.clone(),
                styled_segments: vec![StyledSegment {
                    text: line,
                    style: Style::default(),
                }],
                role,
                separator: false,
                soft_wrap_to_next: wrapped,
            });
        }

        first_physical = false;
    }
}

fn append_wrapped_markdown_lines(
    out: &mut Vec<RenderedLine>,
    role: Role,
    text: &str,
    width: usize,
) {
    if width < 8 {
        return;
    }

    let logical_lines = markdown_line_segments(text);
    for logical in logical_lines {
        let plain = styled_plain_text(&logical);
        if plain.is_empty() {
            out.push(RenderedLine {
                cells: 0,
                text: String::new(),
                styled_segments: Vec::new(),
                role,
                separator: false,
                soft_wrap_to_next: false,
            });
            continue;
        }

        let wrapped_parts = wrap_natural_by_cells(&plain, width);
        let mut remaining: VecDeque<StyledSegment> = logical.into();

        for (i, part) in wrapped_parts.iter().enumerate() {
            let part_cells = visual_width(part);
            let wrapped = i + 1 < wrapped_parts.len();
            let styled_segments = normalize_styled_segments_for_part(
                part,
                take_styled_segments_by_cells(&mut remaining, part_cells),
            );

            out.push(RenderedLine {
                cells: part_cells,
                text: part.clone(),
                styled_segments,
                role,
                separator: false,
                soft_wrap_to_next: wrapped,
            });
        }
    }
}

fn diff_line_style(line: &str) -> Style {
    if line.starts_with("@@") {
        return Style::default()
            .fg(COLOR_DIFF_HUNK)
            .add_modifier(Modifier::BOLD);
    }
    if (line.starts_with("+++") || line.starts_with("---")) && line.len() > 3 {
        return Style::default().fg(COLOR_DIFF_HEADER);
    }
    if line.starts_with("diff --git ")
        || line.starts_with("index ")
        || line.starts_with("new file mode ")
        || line.starts_with("deleted file mode ")
    {
        return Style::default()
            .fg(COLOR_DIFF_HEADER)
            .add_modifier(Modifier::DIM);
    }
    if line.starts_with('+') && !line.starts_with("+++") {
        return Style::default().fg(COLOR_DIFF_ADD);
    }
    if line.starts_with('-') && !line.starts_with("---") {
        return Style::default().fg(COLOR_DIFF_REMOVE);
    }
    Style::default().fg(COLOR_TEXT)
}

fn make_diff_viewer_style() -> DiffViewerStyle {
    DiffViewerStyle {
        border_style: Style::default().fg(COLOR_STEP6),
        line_number_style: Style::default().fg(COLOR_DIM),
        context_style: Style::default().fg(COLOR_TEXT),
        addition_style: Style::default().fg(COLOR_DIFF_ADD),
        addition_bg: Color::Rgb(22, 41, 29),
        deletion_style: Style::default().fg(COLOR_DIFF_REMOVE),
        deletion_bg: Color::Rgb(52, 25, 38),
        inline_addition_style: Style::default().fg(COLOR_STEP1).bg(COLOR_DIFF_ADD),
        inline_deletion_style: Style::default().fg(COLOR_STEP1).bg(COLOR_DIFF_REMOVE),
        hunk_header_style: Style::default()
            .fg(COLOR_DIFF_HUNK)
            .add_modifier(Modifier::BOLD),
        match_style: Style::default().bg(COLOR_STEP6).fg(COLOR_TEXT),
        current_match_style: Style::default().bg(COLOR_PRIMARY).fg(COLOR_STEP1),
        gutter_separator: "│",
        side_separator: "│",
    }
}

fn diff_total_lines(state: &DiffViewerState) -> usize {
    state
        .diff
        .hunks
        .iter()
        .map(|h| h.lines.len() + 1)
        .sum::<usize>()
}

fn trim_right_spaces(styled_segments: &mut Vec<StyledSegment>) {
    while let Some(last) = styled_segments.last_mut() {
        let trimmed_len = last.text.trim_end_matches(' ').len();
        if trimmed_len == 0 {
            styled_segments.pop();
            continue;
        }
        if trimmed_len < last.text.len() {
            last.text.truncate(trimmed_len);
        }
        break;
    }
}

fn rendered_line_from_buffer_row(
    buf: &Buffer,
    y: u16,
    x_start: u16,
    width: u16,
    role: Role,
) -> RenderedLine {
    let mut styled_segments: Vec<StyledSegment> = Vec::new();

    for x in x_start..x_start.saturating_add(width) {
        let Some(cell) = buf.cell((x, y)) else {
            continue;
        };
        let sym = cell.symbol();
        if sym.is_empty() {
            continue;
        }

        let style = cell.style();

        if let Some(last) = styled_segments.last_mut() {
            if last.style == style {
                last.text.push_str(sym);
                continue;
            }
        }
        styled_segments.push(StyledSegment {
            text: sym.to_string(),
            style,
        });
    }

    trim_right_spaces(&mut styled_segments);
    let text = styled_segments
        .iter()
        .map(|s| s.text.as_str())
        .collect::<String>();
    let cells = visual_width(&text);

    RenderedLine {
        text,
        styled_segments,
        role,
        separator: false,
        cells,
        soft_wrap_to_next: false,
    }
}

fn append_diff_viewer_lines(
    out: &mut Vec<RenderedLine>,
    role: Role,
    file_path: Option<&str>,
    diff: &str,
    width: usize,
) -> bool {
    if width < 8 {
        return false;
    }

    let parsed = DiffData::from_unified_diff(diff);
    if parsed.hunks.is_empty() {
        return false;
    }

    let mut staged = Vec::new();

    let diff_path = file_path
        .filter(|p| !p.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| parsed.new_path.clone())
        .or_else(|| parsed.old_path.clone());

    let area_w = match u16::try_from(width.saturating_add(2)) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let content_x = 1u16;
    let content_y = 1u16;
    let content_w = area_w.saturating_sub(2);
    if content_w == 0 {
        return false;
    }

    let hunk_total = parsed.hunks.len();
    for (idx, hunk) in parsed.hunks.iter().enumerate() {
        if idx > 0 {
            staged.push(RenderedLine {
                cells: 0,
                text: String::new(),
                styled_segments: Vec::new(),
                role,
                separator: false,
                soft_wrap_to_next: false,
            });
        }

        if let Some(path) = diff_path.as_deref() {
            if !path.is_empty() {
                staged.push(RenderedLine {
                    cells: visual_width(path),
                    text: path.to_string(),
                    styled_segments: vec![StyledSegment {
                        text: path.to_string(),
                        style: Style::default().fg(COLOR_DIM).add_modifier(Modifier::BOLD),
                    }],
                    role,
                    separator: false,
                    soft_wrap_to_next: false,
                });
            }
        }

        let hunk_label = format!(
            "Hunk {}/{}  old {}..{} -> new {}..{}",
            idx + 1,
            hunk_total,
            hunk.old_start,
            hunk.old_start + hunk.old_count.saturating_sub(1),
            hunk.new_start,
            hunk.new_start + hunk.new_count.saturating_sub(1)
        );
        staged.push(RenderedLine {
            cells: visual_width(&hunk_label),
            text: hunk_label.clone(),
            styled_segments: vec![StyledSegment {
                text: hunk_label,
                style: Style::default()
                    .fg(COLOR_DIFF_HUNK)
                    .add_modifier(Modifier::BOLD),
            }],
            role,
            separator: false,
            soft_wrap_to_next: false,
        });

        let mut one_hunk = DiffData::new(parsed.old_path.clone(), parsed.new_path.clone());
        one_hunk.hunks.push(hunk.clone());

        let mut state = DiffViewerState::new(one_hunk);
        state.view_mode = DiffViewMode::Unified;
        state.show_line_numbers = true;
        let total_lines = diff_total_lines(&state);
        if total_lines == 0 {
            continue;
        }

        let area_h = match u16::try_from(total_lines.saturating_add(3)) {
            Ok(v) => v,
            Err(_) => return false,
        };
        let area = Rect::new(0, 0, area_w, area_h);
        let mut buf = Buffer::empty(area);
        let viewer = DiffViewer::new(&state)
            .show_stats(false)
            .style(make_diff_viewer_style());
        viewer.render(area, &mut buf);

        let content_h = area_h.saturating_sub(3);
        for row in 0..content_h {
            let line =
                rendered_line_from_buffer_row(&buf, content_y + row, content_x, content_w, role);
            if line.text.trim_start().starts_with("@@") {
                continue;
            }
            staged.push(line);
        }
    }

    out.extend(staged);
    true
}

fn append_wrapped_diff_lines(
    out: &mut Vec<RenderedLine>,
    role: Role,
    file_path: Option<&str>,
    diff: &str,
    width: usize,
) {
    if append_diff_viewer_lines(out, role, file_path, diff, width) {
        return;
    }

    if width < 8 {
        return;
    }

    if let Some(path) = file_path {
        if !path.is_empty() {
            out.push(RenderedLine {
                cells: visual_width(path),
                text: path.to_string(),
                styled_segments: vec![StyledSegment {
                    text: path.to_string(),
                    style: Style::default().fg(COLOR_DIM).add_modifier(Modifier::BOLD),
                }],
                role,
                separator: false,
                soft_wrap_to_next: false,
            });
        }
    }

    for logical in diff.split('\n') {
        let line_style = diff_line_style(logical);
        if logical.is_empty() {
            out.push(RenderedLine {
                cells: 0,
                text: String::new(),
                styled_segments: Vec::new(),
                role,
                separator: false,
                soft_wrap_to_next: false,
            });
            continue;
        }

        let wrapped_parts = wrap_natural_by_cells(logical, width);
        for (i, part) in wrapped_parts.iter().enumerate() {
            let wrapped = i + 1 < wrapped_parts.len();
            out.push(RenderedLine {
                cells: visual_width(part),
                text: part.clone(),
                styled_segments: vec![StyledSegment {
                    text: part.clone(),
                    style: line_style,
                }],
                role,
                separator: false,
                soft_wrap_to_next: wrapped,
            });
        }
    }
}

fn read_summary_path(text: &str) -> Option<&str> {
    let trimmed = text.trim();
    let rest = trimmed.strip_prefix("→ Read")?.trim_start();
    if rest.is_empty() {
        return Some("");
    }
    let path = rest.split_once(" [").map(|(p, _)| p).unwrap_or(rest).trim();
    Some(path)
}

fn format_read_summary_with_count(path: &str, count: usize) -> String {
    let base = if path.is_empty() {
        "→ Read".to_string()
    } else {
        format!("→ Read {path}")
    };
    if count > 1 {
        format!("{base} ×{count}")
    } else {
        base
    }
}

fn collapse_successive_read_summaries(messages: &[Message]) -> Vec<Message> {
    let mut out = Vec::with_capacity(messages.len());
    let mut i = 0usize;

    while i < messages.len() {
        let msg = &messages[i];
        let can_collapse = msg.kind == MessageKind::Plain
            && msg.role == Role::ToolCall
            && !msg.text.contains('\n');

        let Some(path) = can_collapse.then(|| read_summary_path(&msg.text)).flatten() else {
            out.push(msg.clone());
            i += 1;
            continue;
        };
        let key = path.to_string();

        let mut count = 1usize;
        let mut j = i + 1;
        while j < messages.len() {
            let next = &messages[j];
            if next.kind != MessageKind::Plain
                || next.role != Role::ToolCall
                || next.text.contains('\n')
            {
                break;
            }
            let Some(next_path) = read_summary_path(&next.text) else {
                break;
            };
            if next_path != key {
                break;
            }
            count += 1;
            j += 1;
        }

        if count > 1 {
            let mut collapsed = msg.clone();
            collapsed.text = format_read_summary_with_count(&key, count);
            out.push(collapsed);
            i = j;
        } else {
            out.push(msg.clone());
            i += 1;
        }
    }

    out
}

fn build_rendered_lines(messages: &[Message], width: usize) -> Vec<RenderedLine> {
    build_rendered_lines_with_hidden(messages, width, None)
}

fn build_rendered_lines_with_hidden(
    messages: &[Message],
    width: usize,
    hidden_user_message_idx: Option<usize>,
) -> Vec<RenderedLine> {
    let mut filtered = Vec::with_capacity(messages.len());
    for (idx, msg) in messages.iter().enumerate() {
        if hidden_user_message_idx == Some(idx) && msg.role == Role::User {
            continue;
        }
        filtered.push(msg.clone());
    }
    let messages = collapse_successive_read_summaries(&filtered);
    let mut out = Vec::new();
    let mut appended_any = false;

    for msg in &messages {
        if appended_any {
            out.push(RenderedLine {
                text: String::new(),
                styled_segments: Vec::new(),
                role: Role::System,
                separator: true,
                cells: 0,
                soft_wrap_to_next: false,
            });
        }
        match msg.kind {
            MessageKind::Diff => append_wrapped_diff_lines(
                &mut out,
                msg.role,
                msg.file_path.as_deref(),
                &msg.text,
                width,
            ),
            MessageKind::Plain => match msg.role {
                Role::Assistant | Role::Reasoning => {
                    append_wrapped_markdown_lines(&mut out, msg.role, &msg.text, width);
                }
                _ => append_wrapped_message_lines(&mut out, msg.role, &msg.text, width),
            },
        }
        appended_any = true;
    }

    out
}

fn transcript_content_width(size: TerminalSize) -> usize {
    if size.width > MSG_CONTENT_X + 1 {
        size.width - (MSG_CONTENT_X + 1)
    } else {
        0
    }
}

#[derive(Debug, Clone)]
struct InputLayout {
    msg_bottom: usize,   // 1-based; 0 means no transcript row is available
    input_top: usize,    // 1-based
    input_height: usize, // rows
    text_width: usize,   // cells available for input text
    visible_lines: Vec<String>,
    cursor_x: usize, // 0-based terminal column
    cursor_y: usize, // 0-based terminal row
}

fn input_cursor_visual_position(
    line: &str,
    cursor_col_chars: usize,
    width: usize,
) -> (usize, usize) {
    if width == 0 {
        return (0, 0);
    }

    let cursor_byte = char_to_byte_idx(line, cursor_col_chars).min(line.len());
    let prefix = &line[..cursor_byte];
    let wrapped_prefix = wrap_input_line(prefix, width);
    let row = wrapped_prefix.len().saturating_sub(1);
    let col = wrapped_prefix
        .last()
        .map(|part| visual_width(part))
        .unwrap_or(0);
    (row, col)
}

fn textarea_input_from_code(code: KeyCode, modifiers: KeyModifiers) -> TextInput {
    let key = match code {
        KeyCode::Char(c) => TextKey::Char(c),
        KeyCode::Backspace => TextKey::Backspace,
        KeyCode::Enter => TextKey::Enter,
        KeyCode::Left => TextKey::Left,
        KeyCode::Right => TextKey::Right,
        KeyCode::Up => TextKey::Up,
        KeyCode::Down => TextKey::Down,
        KeyCode::Tab => TextKey::Tab,
        KeyCode::Delete => TextKey::Delete,
        KeyCode::Home => TextKey::Home,
        KeyCode::End => TextKey::End,
        KeyCode::PageUp => TextKey::PageUp,
        KeyCode::PageDown => TextKey::PageDown,
        KeyCode::Esc => TextKey::Esc,
        KeyCode::F(n) => TextKey::F(n),
        _ => TextKey::Null,
    };

    TextInput {
        key,
        ctrl: modifiers.contains(KeyModifiers::CONTROL),
        alt: modifiers.contains(KeyModifiers::ALT),
        shift: modifiers.contains(KeyModifiers::SHIFT),
    }
}

fn textarea_input_from_key(k: crossterm::event::KeyEvent) -> TextInput {
    textarea_input_from_code(k.code, k.modifiers)
}

fn normalize_pasted_text(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn is_newline_enter(mods: KeyModifiers) -> bool {
    mods.contains(KeyModifiers::SHIFT) || mods.contains(KeyModifiers::ALT)
}

fn compute_input_layout(app: &AppState, size: TerminalSize) -> InputLayout {
    let text_width = transcript_content_width(size);
    let mut wrapped = Vec::new();
    let lines = app.input.lines();

    let (cursor_row, cursor_col_chars) = app.input.cursor();
    let mut cursor_wrapped_row = 0usize;
    let mut cursor_wrapped_col = 0usize;
    let mut cursor_set = false;

    for (row, line) in lines.iter().enumerate() {
        let wrapped_line = wrap_input_line(line, text_width);

        if row < cursor_row {
            cursor_wrapped_row += wrapped_line.len();
        } else if row == cursor_row {
            let (line_row, line_col) =
                input_cursor_visual_position(line, cursor_col_chars, text_width);
            cursor_wrapped_row += line_row;
            cursor_wrapped_col = line_col;
            cursor_set = true;
        }

        wrapped.extend(wrapped_line);
    }

    if wrapped.is_empty() {
        wrapped.push(String::new());
    }

    let mut max_input_rows = 8usize.min(size.height.max(1));
    if size.height > 1 {
        max_input_rows = max_input_rows.min(size.height - 1);
    }

    let input_height = wrapped.len().clamp(1, max_input_rows.max(1));

    if !cursor_set {
        cursor_wrapped_row = wrapped.len().saturating_sub(1);
        cursor_wrapped_col = wrapped.last().map(|line| visual_width(line)).unwrap_or(0);
    }

    let mut visible_start = wrapped.len().saturating_sub(input_height);
    if cursor_wrapped_row < visible_start {
        visible_start = cursor_wrapped_row;
    }
    if cursor_wrapped_row >= visible_start + input_height {
        visible_start = cursor_wrapped_row + 1 - input_height;
    }

    let visible_end = (visible_start + input_height).min(wrapped.len());
    let mut visible_lines = wrapped[visible_start..visible_end].to_vec();
    while visible_lines.len() < input_height {
        visible_lines.insert(0, String::new());
    }

    let input_top = size.height + 1 - input_height;
    let msg_bottom = input_top.saturating_sub(2);
    let cursor_visual_row = cursor_wrapped_row.saturating_sub(visible_start);
    let cursor_x = MSG_CONTENT_X + cursor_wrapped_col;
    let cursor_y = input_top.saturating_sub(1) + cursor_visual_row.min(input_height - 1);

    InputLayout {
        msg_bottom,
        input_top,
        input_height,
        text_width,
        visible_lines,
        cursor_x,
        cursor_y,
    }
}

fn last_assistant_message(messages: &[Message]) -> Option<&str> {
    messages
        .iter()
        .rev()
        .find(|m| m.role == Role::Assistant && !m.text.is_empty())
        .map(|m| m.text.as_str())
}

#[derive(Debug, Clone, Copy)]
struct PickerLayout {
    panel_x: usize,
    panel_y: usize,
    panel_w: usize,
    panel_h: usize,
    list_x: usize,
    list_y: usize,
    list_w: usize,
    list_h: usize,
}

fn compute_picker_layout(size: TerminalSize) -> PickerLayout {
    let panel_w = if size.width > 6 {
        (size.width - 6).min(92)
    } else {
        size.width
    };
    let panel_h = if size.height > 4 {
        (size.height - 2).min(24)
    } else {
        size.height
    };
    let panel_x = if size.width > panel_w {
        (size.width - panel_w) / 2
    } else {
        0
    };
    let panel_y = if size.height > panel_h {
        (size.height - panel_h) / 3
    } else {
        0
    };

    let list_x = panel_x + 2;
    let list_y = panel_y + 3;
    let list_w = if panel_w > 4 { panel_w - 4 } else { 0 };
    let list_h = if panel_h > 6 { panel_h - 6 } else { 1 };

    PickerLayout {
        panel_x,
        panel_y,
        panel_w,
        panel_h,
        list_x,
        list_y,
        list_w,
        list_h,
    }
}

fn draw_str(buf: &mut Buffer, x: usize, y: usize, text: &str, style: Style, max_width: usize) {
    if text.is_empty() || max_width == 0 {
        return;
    }
    if let (Ok(x), Ok(y), Ok(w)) = (
        u16::try_from(x),
        u16::try_from(y),
        usize::try_from(max_width),
    ) {
        buf.set_stringn(x, y, text, w, style);
    }
}

fn fill_rect(buf: &mut Buffer, x: usize, y: usize, w: usize, h: usize, style: Style) {
    if w == 0 || h == 0 {
        return;
    }
    if let (Ok(x), Ok(y), Ok(w), Ok(h)) = (
        u16::try_from(x),
        u16::try_from(y),
        u16::try_from(w),
        u16::try_from(h),
    ) {
        buf.set_style(
            Rect {
                x,
                y,
                width: w,
                height: h,
            },
            style,
        );
    }
}

fn draw_rendered_line(
    buf: &mut Buffer,
    x: usize,
    y: usize,
    max_width: usize,
    line: &RenderedLine,
    base_style: Style,
    selection: Option<(usize, usize)>,
) {
    if max_width == 0 || line.cells == 0 {
        return;
    }

    if !line.styled_segments.is_empty() {
        draw_str(buf, x, y, &line.text, base_style, max_width);
    }

    let mut draw_x = x;
    let mut col = 0usize;

    let mut render_segment = |text: &str, seg_style: Style, draw_x: &mut usize, col: &mut usize| {
        if *draw_x >= x + max_width || text.is_empty() {
            return;
        }

        let seg_cells = visual_width(text);
        if seg_cells == 0 {
            return;
        }

        let style = base_style.patch(seg_style);
        let seg_start = *col;
        let seg_end = seg_start + seg_cells;

        let mut draw_piece = |piece: &str, piece_style: Style, draw_x: &mut usize| {
            if piece.is_empty() || *draw_x >= x + max_width {
                return;
            }
            let rem = max_width.saturating_sub(*draw_x - x);
            draw_str(buf, *draw_x, y, piece, piece_style, rem);
            *draw_x += visual_width(piece);
        };

        if let Some((sel_start, sel_end)) = selection {
            if sel_end <= seg_start || sel_start >= seg_end {
                draw_piece(text, style, draw_x);
            } else {
                let local_start = sel_start.saturating_sub(seg_start).min(seg_cells);
                let local_end = sel_end.saturating_sub(seg_start).min(seg_cells);

                let before = slice_by_cells(text, 0, local_start);
                let selected = slice_by_cells(text, local_start, local_end);
                let after = slice_by_cells(text, local_end, seg_cells);

                draw_piece(&before, style, draw_x);
                draw_piece(&selected, style.fg(COLOR_TEXT).bg(COLOR_STEP8), draw_x);
                draw_piece(&after, style, draw_x);
            }
        } else {
            draw_piece(text, style, draw_x);
        }

        *col = seg_end;
    };

    if line.styled_segments.is_empty() {
        render_segment(&line.text, Style::default(), &mut draw_x, &mut col);
        return;
    }

    for seg in &line.styled_segments {
        if draw_x >= x + max_width {
            break;
        }
        render_segment(&seg.text, seg.style, &mut draw_x, &mut col);
    }

    // Some markdown renderers occasionally leave a trailing token outside
    // styled spans; render the uncovered tail from canonical line text.
    if col < line.cells && draw_x < x + max_width {
        let tail = slice_by_cells(&line.text, col, line.cells);
        render_segment(&tail, Style::default(), &mut draw_x, &mut col);
    }
}

fn draw_help_overlay(buf: &mut Buffer, size: TerminalSize) {
    if !(size.height > 10 && size.width > 44) {
        return;
    }

    let box_w = (size.width - 8).min(74);
    let box_h = 10usize;
    let start_x = (size.width - box_w) / 2;
    let start_y = (size.height - box_h) / 2;

    fill_rect(
        buf,
        0,
        0,
        size.width,
        size.height,
        Style::default().bg(COLOR_OVERLAY),
    );
    fill_rect(
        buf,
        start_x,
        start_y,
        box_w,
        box_h,
        Style::default().bg(COLOR_STEP2),
    );

    let left = start_x;
    let right = start_x + box_w - 1;
    let top = start_y;
    let bottom = start_y + box_h - 1;

    draw_str(buf, left, top, "┏", Style::default().fg(COLOR_STEP7), 1);
    draw_str(buf, right, top, "┓", Style::default().fg(COLOR_STEP7), 1);
    draw_str(buf, left, bottom, "┗", Style::default().fg(COLOR_STEP7), 1);
    draw_str(buf, right, bottom, "┛", Style::default().fg(COLOR_STEP7), 1);

    for x in (left + 1)..right {
        draw_str(buf, x, top, "─", Style::default().fg(COLOR_STEP7), 1);
        draw_str(buf, x, bottom, "─", Style::default().fg(COLOR_STEP7), 1);
    }
    for y in (top + 1)..bottom {
        draw_str(buf, left, y, "┃", Style::default().fg(COLOR_STEP7), 1);
        draw_str(buf, right, y, "┃", Style::default().fg(COLOR_STEP7), 1);
    }

    draw_str(
        buf,
        start_x + 3,
        start_y + 1,
        "Help",
        Style::default()
            .fg(COLOR_PRIMARY)
            .add_modifier(Modifier::BOLD),
        box_w.saturating_sub(6),
    );
    draw_str(
        buf,
        start_x + box_w.saturating_sub(8),
        start_y + 1,
        "esc",
        Style::default().fg(COLOR_DIM),
        3,
    );
    draw_str(
        buf,
        start_x + 3,
        start_y + 3,
        "Enter send/steer  Shift/Alt+Enter newline",
        Style::default().fg(COLOR_TEXT),
        box_w.saturating_sub(6),
    );
    draw_str(
        buf,
        start_x + 3,
        start_y + 4,
        "Ctrl+Y copy selection or last answer",
        Style::default().fg(COLOR_TEXT),
        box_w.saturating_sub(6),
    );
    draw_str(
        buf,
        start_x + 3,
        start_y + 5,
        "g/G or Home/End jump transcript",
        Style::default().fg(COLOR_TEXT),
        box_w.saturating_sub(6),
    );
    draw_str(
        buf,
        start_x + 3,
        start_y + 6,
        "Wheel scroll, drag to select, release to copy",
        Style::default().fg(COLOR_TEXT),
        box_w.saturating_sub(6),
    );
    draw_str(
        buf,
        start_x + 3,
        start_y + 7,
        "? toggle this help",
        Style::default().fg(COLOR_DIM),
        box_w.saturating_sub(6),
    );
}

fn draw_perf_overlay(buf: &mut Buffer, size: TerminalSize, perf: &PerfMetrics) {
    let lines = perf.overlay_lines();
    if lines.is_empty() || size.width < 44 || size.height < lines.len() + 4 {
        return;
    }

    let inner_w = lines
        .iter()
        .map(|line| visual_width(line))
        .max()
        .unwrap_or(0)
        .min(size.width.saturating_sub(6));
    if inner_w == 0 {
        return;
    }

    let box_w = inner_w + 4;
    let box_h = lines.len() + 2;
    let start_x = size.width.saturating_sub(box_w + 2);
    let start_y = 1usize;
    if start_y + box_h > size.height {
        return;
    }

    fill_rect(
        buf,
        start_x,
        start_y,
        box_w,
        box_h,
        Style::default().bg(COLOR_STEP1),
    );

    let left = start_x;
    let right = start_x + box_w - 1;
    let top = start_y;
    let bottom = start_y + box_h - 1;

    draw_str(buf, left, top, "┌", Style::default().fg(COLOR_STEP7), 1);
    draw_str(buf, right, top, "┐", Style::default().fg(COLOR_STEP7), 1);
    draw_str(buf, left, bottom, "└", Style::default().fg(COLOR_STEP7), 1);
    draw_str(buf, right, bottom, "┘", Style::default().fg(COLOR_STEP7), 1);

    for x in (left + 1)..right {
        draw_str(buf, x, top, "─", Style::default().fg(COLOR_STEP7), 1);
        draw_str(buf, x, bottom, "─", Style::default().fg(COLOR_STEP7), 1);
    }
    for y in (top + 1)..bottom {
        draw_str(buf, left, y, "│", Style::default().fg(COLOR_STEP7), 1);
        draw_str(buf, right, y, "│", Style::default().fg(COLOR_STEP7), 1);
    }

    for (i, line) in lines.iter().enumerate() {
        draw_str(
            buf,
            start_x + 2,
            start_y + 1 + i,
            line,
            Style::default().fg(COLOR_DIM),
            inner_w,
        );
    }
}

fn render_main_view(frame: &mut ratatui::Frame<'_>, app: &mut AppState) {
    let area = frame.area();
    let size = TerminalSize {
        width: area.width as usize,
        height: area.height as usize,
    };

    if size.width == 0 || size.height == 0 {
        return;
    }

    let input_layout_started = Instant::now();
    let input_layout = compute_input_layout(app, size);
    if let Some(perf) = app.perf.as_mut() {
        perf.input_layout.push(input_layout_started.elapsed());
    }
    let msg_top = MSG_TOP;
    let msg_bottom = input_layout.msg_bottom;
    let msg_height = if msg_bottom >= msg_top {
        msg_bottom - msg_top + 1
    } else {
        0
    };
    let msg_width = transcript_content_width(size);

    let total_lines = app.rendered_lines.len();
    let max_scroll = total_lines.saturating_sub(msg_height);
    if app.scroll_top > max_scroll {
        app.scroll_top = max_scroll;
    }
    if app.auto_follow_bottom && max_scroll > 0 {
        app.scroll_top = max_scroll;
    }

    let buf = frame.buffer_mut();
    fill_rect(
        buf,
        0,
        0,
        size.width,
        size.height,
        Style::default().bg(COLOR_STEP1),
    );
    if msg_height > 0 {
        fill_rect(
            buf,
            0,
            msg_top - 1,
            size.width,
            msg_height,
            Style::default().bg(COLOR_STEP2),
        );
    }

    for i in 0..msg_height {
        let line_idx = app.scroll_top + i;
        let row_1b = msg_top + i;
        let y = row_1b - 1;

        let line_opt = app.rendered_lines.get(line_idx);
        if let Some(line) = line_opt {
            if !line.separator {
                let gutter_symbol = role_gutter_symbol(line.role);
                draw_str(
                    buf,
                    0,
                    y,
                    gutter_symbol,
                    Style::default()
                        .fg(role_gutter_fg(line.role))
                        .add_modifier(Modifier::BOLD),
                    1,
                );
            }
        }

        if msg_width == 0 {
            continue;
        }

        let Some(line) = line_opt else {
            continue;
        };
        fill_rect(
            buf,
            MSG_CONTENT_X,
            y,
            msg_width,
            1,
            Style::default().bg(role_row_bg(line.role)),
        );
        if line.separator {
            let sep = "─".repeat(msg_width);
            draw_str(
                buf,
                MSG_CONTENT_X,
                y,
                &sep,
                Style::default().fg(COLOR_STEP6),
                msg_width,
            );
            continue;
        }

        let mut base_style = Style::default().fg(role_fg(line.role));
        if matches!(line.role, Role::Reasoning) {
            base_style = base_style.add_modifier(Modifier::DIM);
        }

        let selection_range = app
            .selection
            .and_then(|sel| compute_selection_range(sel, row_1b, line.cells))
            .map(|(start, end)| (start.min(line.cells), end.min(line.cells)));

        draw_rendered_line(
            buf,
            MSG_CONTENT_X,
            y,
            msg_width,
            line,
            base_style,
            selection_range,
        );
    }

    if input_layout.input_top > 1 {
        let sep_y = input_layout.input_top - 2;
        if size.width > 0 {
            let working = app.active_turn_id.is_some();
            let line_len = size.width.saturating_sub(1);
            let context_label = app
                .context_usage
                .map(context_usage_label)
                .unwrap_or_else(|| context_usage_placeholder_label().to_string());
            let has_context_usage = app.context_usage.is_some();
            let reserved_label_cells = context_label_reserved_cells(Some(&context_label));
            let context_label_cells = visual_width(&context_label);
            let can_reserve_label_area = reserved_label_cells + 1 < line_len;
            let label_area_start = if can_reserve_label_area {
                line_len - reserved_label_cells
            } else {
                line_len
            };
            let anim_end = if can_reserve_label_area {
                label_area_start.saturating_sub(1)
            } else {
                line_len
            };
            let tick = animation_tick();
            let head = if anim_end > 0 {
                kitt_head_index(anim_end, tick)
            } else {
                0
            };
            if anim_end > 0 {
                if app.rewind_mode {
                    let sep = "━".repeat(anim_end);
                    draw_str(
                        buf,
                        0,
                        sep_y,
                        &sep,
                        Style::default().fg(COLOR_DIFF_REMOVE),
                        anim_end,
                    );
                } else if working {
                    for x in 0..anim_end {
                        let dist = head.abs_diff(x);
                        draw_str(
                            buf,
                            x,
                            sep_y,
                            "━",
                            Style::default().fg(kitt_color_for_distance(dist)),
                            1,
                        );
                    }
                } else {
                    let sep = "━".repeat(anim_end);
                    draw_str(
                        buf,
                        0,
                        sep_y,
                        &sep,
                        Style::default().fg(COLOR_GUTTER_USER),
                        anim_end,
                    );
                }
            }

            if can_reserve_label_area && context_label_cells > 0 {
                let label_x = line_len.saturating_sub(context_label_cells);
                draw_str(
                    buf,
                    label_x,
                    sep_y,
                    &context_label,
                    Style::default().fg(if has_context_usage {
                        COLOR_STEP8
                    } else {
                        COLOR_STEP7
                    }),
                    context_label_cells,
                );
            }
        }
    }

    fill_rect(
        buf,
        0,
        input_layout.input_top.saturating_sub(1),
        size.width,
        input_layout.input_height,
        Style::default().bg(COLOR_STEP3),
    );
    for i in 0..input_layout.input_height {
        let y = input_layout.input_top.saturating_sub(1) + i;
        let input_gutter_color = if app.rewind_mode {
            COLOR_DIFF_REMOVE
        } else {
            COLOR_GUTTER_USER
        };
        draw_str(
            buf,
            0,
            y,
            ">",
            Style::default()
                .fg(input_gutter_color)
                .add_modifier(Modifier::BOLD),
            1,
        );
        if let Some(line) = input_layout.visible_lines.get(i) {
            draw_str(
                buf,
                MSG_CONTENT_X,
                y,
                line,
                Style::default().fg(COLOR_TEXT),
                input_layout.text_width,
            );
        }
    }

    if app.show_help {
        draw_help_overlay(buf, size);
    }
    if let Some(perf) = app.perf.as_ref() {
        if perf.show_overlay {
            draw_perf_overlay(buf, size, perf);
        }
    }

    let cursor_x = input_layout.cursor_x.min(size.width.saturating_sub(2));
    let cursor_y = input_layout.cursor_y.min(size.height.saturating_sub(1));
    frame.set_cursor_position((cursor_x as u16, cursor_y as u16));
}

fn draw_picker(
    frame: &mut ratatui::Frame<'_>,
    threads: &[ThreadSummary],
    selected: usize,
    top: usize,
) {
    let area = frame.area();
    let size = TerminalSize {
        width: area.width as usize,
        height: area.height as usize,
    };

    if size.width == 0 || size.height == 0 {
        return;
    }

    let layout = compute_picker_layout(size);
    let buf = frame.buffer_mut();

    fill_rect(
        buf,
        0,
        0,
        size.width,
        size.height,
        Style::default().bg(COLOR_STEP1),
    );
    fill_rect(
        buf,
        0,
        0,
        size.width,
        size.height,
        Style::default().bg(COLOR_OVERLAY),
    );
    fill_rect(
        buf,
        layout.panel_x,
        layout.panel_y,
        layout.panel_w,
        layout.panel_h,
        Style::default().bg(COLOR_STEP2),
    );

    if layout.panel_w < 8 || layout.panel_h < 7 || layout.list_w == 0 {
        draw_str(
            buf,
            1,
            0,
            "carlos resume",
            Style::default().fg(COLOR_TEXT).add_modifier(Modifier::BOLD),
            size.width.saturating_sub(1),
        );
        return;
    }

    for y in layout.panel_y..(layout.panel_y + layout.panel_h) {
        draw_str(
            buf,
            layout.panel_x,
            y,
            "┃",
            Style::default().fg(COLOR_STEP7),
            1,
        );
        if layout.panel_w > 1 {
            draw_str(
                buf,
                layout.panel_x + layout.panel_w - 1,
                y,
                "┃",
                Style::default().fg(COLOR_STEP7),
                1,
            );
        }
    }

    draw_str(
        buf,
        layout.list_x,
        layout.panel_y + 1,
        "Sessions",
        Style::default().fg(COLOR_TEXT).add_modifier(Modifier::BOLD),
        layout.list_w,
    );
    if layout.panel_w > 5 {
        draw_str(
            buf,
            layout.panel_x + layout.panel_w - 5,
            layout.panel_y + 1,
            "esc",
            Style::default().fg(COLOR_DIM),
            3,
        );
    }
    draw_str(
        buf,
        layout.list_x,
        layout.panel_y + 2,
        "Enter open  j/k move  g/G jump",
        Style::default().fg(COLOR_DIM),
        layout.list_w,
    );

    for row in 0..layout.list_h {
        let idx = top + row;
        let y = layout.list_y + row;
        if idx >= threads.len() {
            continue;
        }

        let t = &threads[idx];
        let preview_w = if layout.list_w > 42 {
            layout.list_w - 42
        } else {
            10
        };
        let preview = if visual_width(&t.preview) > preview_w {
            let cut = split_at_cells(&t.preview, preview_w);
            &t.preview[..cut]
        } else {
            &t.preview
        };

        let cwd_tail = if t.cwd.is_empty() { "" } else { &t.cwd };
        let mut line = format!("{}  {}  {}", t.id, preview, cwd_tail);
        if !line.is_empty() {
            let ts = t.updated_at;
            line.push_str(&format!("  {}", ts));
        }
        let view_width = layout.list_w.saturating_sub(2);
        if visual_width(&line) > view_width {
            let cut = split_at_cells(&line, view_width);
            line.truncate(cut);
        }

        let active = idx == selected;
        if active && layout.list_w > 0 {
            fill_rect(
                buf,
                layout.list_x,
                y,
                layout.list_w,
                1,
                Style::default().bg(COLOR_PRIMARY),
            );
        }

        let bullet_style = if active {
            Style::default().fg(COLOR_STEP1).bg(COLOR_PRIMARY)
        } else {
            Style::default().fg(COLOR_DIM)
        };
        let line_style = if active {
            Style::default()
                .fg(COLOR_STEP1)
                .bg(COLOR_PRIMARY)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(COLOR_TEXT)
        };

        draw_str(
            buf,
            layout.list_x,
            y,
            if active { "●" } else { " " },
            bullet_style,
            1,
        );
        draw_str(buf, layout.list_x + 2, y, &line, line_style, view_width);
    }

    draw_str(
        buf,
        layout.list_x,
        layout.panel_y + layout.panel_h - 2,
        &format!("{} sessions", threads.len()),
        Style::default().fg(COLOR_DIM),
        layout.list_w,
    );
}

fn is_ctrl_char(code: KeyCode, modifiers: KeyModifiers, ch: char) -> bool {
    matches!(code, KeyCode::Char(c) if c.eq_ignore_ascii_case(&ch))
        && modifiers.contains(KeyModifiers::CONTROL)
}

fn is_perf_toggle_key(code: KeyCode, modifiers: KeyModifiers) -> bool {
    matches!(code, KeyCode::F(8)) || is_ctrl_char(code, modifiers, 'p')
}

fn animation_tick() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() / KITT_STEP_MS)
        .unwrap_or(0)
}

fn animation_poll_timeout(working: bool) -> Duration {
    if !working {
        return Duration::from_millis(25);
    }

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let rem = KITT_STEP_MS - (now_ms % KITT_STEP_MS);
    Duration::from_millis(rem.max(1) as u64)
}

fn kitt_head_index(width: usize, tick: u128) -> usize {
    if width <= 1 {
        return 0;
    }

    let span = (width - 1) as u128;
    let cycle = span * 2;
    if cycle == 0 {
        return 0;
    }

    let phase = tick % cycle;
    if phase <= span {
        phase as usize
    } else {
        (cycle - phase) as usize
    }
}

fn parse_mobile_mouse_coords(s: &str) -> Option<(usize, usize)> {
    if !s.contains(';') {
        return None;
    }

    let nums: Vec<usize> = s
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<usize>().ok())
        .collect();
    if nums.len() < 2 {
        return None;
    }

    Some((nums[nums.len() - 2], nums[nums.len() - 1]))
}

fn apply_mobile_mouse_scroll(app: &mut AppState, y: usize) {
    if let Some(prev) = app.mobile_mouse_last_y {
        app.auto_follow_bottom = false;
        let step = y.abs_diff(prev).min(8);
        if y > prev {
            app.scroll_top = app.scroll_top.saturating_add(step.max(1));
        } else if y < prev {
            app.scroll_top = app.scroll_top.saturating_sub(step.max(1));
        }
    }
    app.mobile_mouse_last_y = Some(y);
}

fn consume_mobile_mouse_char(app: &mut AppState, c: char) -> bool {
    if !app.input_is_empty() {
        app.mobile_mouse_buffer.clear();
        return false;
    }

    if app.mobile_mouse_buffer.is_empty() {
        if c != '<' {
            return false;
        }
        app.mobile_mouse_buffer.push(c);
        return true;
    }

    if c.is_ascii_digit() || c == ';' || c == 'M' || c == 'm' {
        app.mobile_mouse_buffer.push(c);
        if let Some((_, y)) = parse_mobile_mouse_coords(&app.mobile_mouse_buffer) {
            apply_mobile_mouse_scroll(app, y);
            app.mobile_mouse_buffer.clear();
        } else if app.mobile_mouse_buffer.len() > 24 {
            app.mobile_mouse_buffer.clear();
        }
        return true;
    }

    app.mobile_mouse_buffer.clear();
    false
}

fn is_key_press_like(kind: KeyEventKind) -> bool {
    matches!(kind, KeyEventKind::Press | KeyEventKind::Repeat)
}

fn handle_notification_line(app: &mut AppState, line: &str) {
    let Ok(parsed) = serde_json::from_str::<Value>(line) else {
        return;
    };
    let Some(method) = parsed.get("method").and_then(Value::as_str) else {
        return;
    };
    let Some(params) = parsed.get("params").and_then(Value::as_object) else {
        return;
    };

    match method {
        "thread/tokenUsage/updated" => {
            if let Some(usage) = context_usage_from_thread_token_usage_params(params) {
                app.context_usage = Some(usage);
            }
        }
        "thread/compacted" => {
            app.append_context_compacted_marker();
        }
        "turn/started" => {
            app.context_usage = None;
            if let Some(id) = params
                .get("turn")
                .and_then(Value::as_object)
                .and_then(|t| t.get("id"))
                .and_then(Value::as_str)
            {
                app.active_turn_id = Some(id.to_string());
                app.auto_follow_bottom = true;
                app.set_status("turn started");
            }
        }
        "turn/completed" => {
            app.active_turn_id = None;
            let turn_status = params
                .get("turn")
                .and_then(Value::as_object)
                .and_then(|turn| turn.get("status"))
                .and_then(Value::as_str);
            if turn_status == Some("interrupted") {
                app.append_turn_interrupted_marker();
                app.set_status("turn interrupted");
            } else {
                app.set_status("turn completed");
            }
        }
        "turn/diff/updated" => {
            if let (Some(turn_id), Some(diff)) = (
                params.get("turnId").and_then(Value::as_str),
                params.get("diff").and_then(Value::as_str),
            ) {
                app.upsert_turn_diff(turn_id, diff);
            }
        }
        "codex/event/turn_diff" => {
            let turn_id = params
                .get("id")
                .and_then(Value::as_str)
                .or_else(|| params.get("turnId").and_then(Value::as_str));
            let diff = params
                .get("msg")
                .and_then(|m| m.get("unified_diff"))
                .and_then(Value::as_str)
                .or_else(|| params.get("diff").and_then(Value::as_str));
            if let (Some(turn_id), Some(diff)) = (turn_id, diff) {
                app.upsert_turn_diff(turn_id, diff);
            }
        }
        "codex/event/exec_command_end" => {
            if let Some(msg) = params.get("msg") {
                if let Some((call_id, summary)) = command_summary_from_parsed_cmd(msg) {
                    app.set_command_override(&call_id, summary);
                }
            }
        }
        "codex/event/token_count" => {
            if let Some(usage) = context_usage_from_token_count_params(params) {
                app.context_usage = Some(usage);
            }
        }
        "codex/event/raw_response_item" => {
            if let Some(item) = params.get("msg").and_then(|m| m.get("item")) {
                handle_raw_response_item(app, item);
            }
        }
        "item/started" => {
            let Some(item) = params.get("item").and_then(Value::as_object) else {
                return;
            };
            let Some(t) = item.get("type").and_then(Value::as_str) else {
                return;
            };

            match t {
                "userMessage" => {
                    let item_value = Value::Object(item.clone());
                    if let Some(text) = item_text_from_content(&item_value) {
                        let idx = app.append_message(Role::User, text.clone());
                        app.record_input_history(&text, Some(idx));
                    }
                }
                "agentMessage" => {
                    if let Some(id) = item.get("id").and_then(Value::as_str) {
                        let idx = app.append_message(Role::Assistant, String::new());
                        app.put_agent_item_mapping(id, idx);
                    }
                }
                "reasoning" => {
                    if let Some(id) = item.get("id").and_then(Value::as_str) {
                        let idx = app.append_message(Role::Reasoning, String::new());
                        app.put_agent_item_mapping(id, idx);
                    }
                }
                "commandExecution" => {
                    if let Some(id) = item.get("id").and_then(Value::as_str) {
                        if app.agent_item_to_index.contains_key(id) {
                            return;
                        }
                        let idx = app.append_message(Role::ToolCall, String::new());
                        app.put_agent_item_mapping(id, idx);
                    }
                }
                t if is_tool_call_type(t) => {
                    if let Some(id) = item.get("id").and_then(Value::as_str) {
                        if app.agent_item_to_index.contains_key(id) {
                            return;
                        }
                        let idx = app.append_message(Role::ToolCall, String::new());
                        app.put_agent_item_mapping(id, idx);
                    }
                }
                t if is_tool_output_type(t) => {
                    if let Some(id) = item.get("id").and_then(Value::as_str) {
                        if app.agent_item_to_index.contains_key(id) {
                            return;
                        }
                        let idx = app.append_message(Role::ToolOutput, String::new());
                        app.put_agent_item_mapping(id, idx);
                    }
                }
                _ => {}
            }
        }
        "item/completed" => {
            let Some(item) = params.get("item").and_then(Value::as_object) else {
                return;
            };
            let Some(kind) = item.get("type").and_then(Value::as_str) else {
                return;
            };
            if kind == "contextCompaction" {
                app.append_context_compacted_marker();
                return;
            }
            let Some(mut role) = role_for_tool_type(kind) else {
                return;
            };
            if kind == "commandExecution" {
                role = Role::ToolOutput;
            }

            let item_value = Value::Object(item.clone());
            let diffs = extract_diff_blocks(&item_value);
            if diffs.is_empty() {
                let item_id = item.get("id").and_then(Value::as_str);
                let exit_code = first_i64_at_paths(&item_value, &[&["exitCode"], &["exit_code"]]);
                let summary_override =
                    item_id.and_then(|id| app.command_render_overrides.get(id).cloned());
                if let (Some(id), Some(summary)) = (item_id, summary_override.clone()) {
                    if exit_code.unwrap_or(0) == 0 {
                        if let Some(idx) = app.agent_item_to_index.get(id).copied() {
                            if let Some(msg) = app.messages.get_mut(idx) {
                                msg.role = Role::ToolCall;
                                msg.text = summary;
                                msg.kind = MessageKind::Plain;
                                msg.file_path = None;
                            }
                            app.mark_transcript_dirty();
                            app.auto_follow_bottom = true;
                            return;
                        }
                        app.append_message(Role::ToolCall, summary);
                        app.auto_follow_bottom = true;
                        return;
                    }
                }

                if let Some(diff) = command_execution_diff_output(&item_value) {
                    if let Some(id) = item_id {
                        if let Some(idx) = app.agent_item_to_index.get(id).copied() {
                            if let Some(msg) = app.messages.get_mut(idx) {
                                msg.role = role;
                                msg.text = diff;
                                msg.kind = MessageKind::Diff;
                                msg.file_path = None;
                            }
                            app.mark_transcript_dirty();
                            app.auto_follow_bottom = true;
                            return;
                        }
                    }
                    app.append_diff_message(role, None, diff);
                    app.auto_follow_bottom = true;
                    return;
                }

                if let Some(formatted) = format_tool_item(&item_value, role) {
                    let text = if exit_code.unwrap_or(0) != 0 {
                        if let Some(summary) = summary_override {
                            format!("{summary}\n{formatted}")
                        } else {
                            formatted
                        }
                    } else {
                        formatted
                    };
                    let item_id = item.get("id").and_then(Value::as_str);
                    if let Some(id) = item_id {
                        if let Some(idx) = app.agent_item_to_index.get(id).copied() {
                            if let Some(msg) = app.messages.get_mut(idx) {
                                msg.role = role;
                                msg.text = text;
                                msg.kind = MessageKind::Plain;
                                msg.file_path = None;
                            }
                            app.mark_transcript_dirty();
                            app.auto_follow_bottom = true;
                            return;
                        }
                    }
                    app.append_message(role, text);
                    app.auto_follow_bottom = true;
                }
                return;
            }

            let item_id = item.get("id").and_then(Value::as_str);
            if let Some(id) = item_id {
                if let Some(idx) = app.agent_item_to_index.get(id).copied() {
                    if let Some(first) = diffs.first() {
                        if let Some(msg) = app.messages.get_mut(idx) {
                            msg.role = role;
                            msg.text = first.diff.clone();
                            msg.kind = MessageKind::Diff;
                            msg.file_path = first.file_path.clone();
                        }
                        app.mark_transcript_dirty();
                        for block in diffs.iter().skip(1) {
                            app.append_diff_message(
                                role,
                                block.file_path.clone(),
                                block.diff.clone(),
                            );
                        }
                        app.auto_follow_bottom = true;
                        return;
                    }
                }
            }

            for block in diffs {
                app.append_diff_message(role, block.file_path, block.diff);
            }
            app.auto_follow_bottom = true;
        }
        "item/agentMessage/delta" => {
            if let (Some(item_id), Some(delta)) = (
                params.get("itemId").and_then(Value::as_str),
                params.get("delta").and_then(Value::as_str),
            ) {
                app.upsert_agent_delta(item_id, delta);
                app.auto_follow_bottom = true;
            }
        }
        "item/reasoning/summaryTextDelta" => {
            if let (Some(item_id), Some(delta)) = (
                params.get("itemId").and_then(Value::as_str),
                params.get("delta").and_then(Value::as_str),
            ) {
                app.upsert_agent_delta(item_id, delta);
            }
        }
        "item/toolCall/delta"
        | "item/tool_call/delta"
        | "item/toolInvocation/delta"
        | "item/functionCall/delta"
        | "item/mcpToolCall/delta"
        | "item/toolResult/delta"
        | "item/toolOutput/delta"
        | "item/tool_result/delta"
        | "item/functionCallOutput/delta"
        | "item/mcpToolResult/delta" => {
            if let (Some(item_id), Some(delta)) = (
                params.get("itemId").and_then(Value::as_str),
                params.get("delta").and_then(Value::as_str),
            ) {
                app.upsert_agent_delta(item_id, delta);
                app.auto_follow_bottom = true;
            }
        }
        "error" => {
            let msg = params
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("server error");
            app.set_status(msg);
        }
        _ => {}
    }
}

fn with_terminal<T>(
    f: impl FnOnce(&mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<T>,
) -> Result<T> {
    enable_raw_mode().context("failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableBracketedPaste
    )
    .context("failed to enter alt screen")?;

    // Probe kitty keyboard protocol flags after entering alt screen.
    let keyboard_enhancement_enabled = execute!(
        stdout,
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
                | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
        )
    )
    .is_ok();

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("failed to create terminal")?;

    let result = f(&mut terminal);

    let _ = disable_raw_mode();
    if keyboard_enhancement_enabled {
        let _ = execute!(terminal.backend_mut(), PopKeyboardEnhancementFlags);
    }
    let _ = execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableBracketedPaste,
        DisableMouseCapture
    );
    let _ = terminal.show_cursor();

    result
}

fn pick_thread(threads: &[ThreadSummary]) -> Result<Option<String>> {
    if threads.is_empty() {
        return Ok(None);
    }

    with_terminal(|terminal| {
        let mut selected = 0usize;
        let mut top = 0usize;
        let mut last_size = TerminalSize {
            width: 0,
            height: 0,
        };

        loop {
            terminal.draw(|frame| {
                let area = frame.area();
                let size = TerminalSize {
                    width: area.width as usize,
                    height: area.height as usize,
                };

                if size.width != last_size.width || size.height != last_size.height {
                    last_size = size;
                }

                let layout = compute_picker_layout(size);
                let list_height = layout.list_h.max(1);
                if selected < top {
                    top = selected;
                }
                if selected >= top + list_height {
                    top = selected + 1 - list_height;
                }

                draw_picker(frame, threads, selected, top);
            })?;

            if !event::poll(Duration::from_millis(15))? {
                continue;
            }

            let ev = event::read()?;
            match ev {
                Event::Key(k) if is_key_press_like(k.kind) => match (k.code, k.modifiers) {
                    (code, mods) if is_ctrl_char(code, mods, 'c') => return Ok(None),
                    (KeyCode::Esc, _) => return Ok(None),
                    (KeyCode::Up, _) => {
                        selected = selected.saturating_sub(1);
                    }
                    (KeyCode::Down, _) => {
                        if selected + 1 < threads.len() {
                            selected += 1;
                        }
                    }
                    (KeyCode::PageUp, _) => {
                        selected = selected.saturating_sub(10);
                    }
                    (KeyCode::PageDown, _) => {
                        selected = (selected + 10).min(threads.len().saturating_sub(1));
                    }
                    (KeyCode::Home, _) | (KeyCode::Char('g'), _) => selected = 0,
                    (KeyCode::End, _) | (KeyCode::Char('G'), _) => {
                        selected = threads.len().saturating_sub(1)
                    }
                    (KeyCode::Char('j'), _) => {
                        if selected + 1 < threads.len() {
                            selected += 1;
                        }
                    }
                    (KeyCode::Char('k'), _) => {
                        selected = selected.saturating_sub(1);
                    }
                    (KeyCode::Enter, _) => return Ok(Some(threads[selected].id.clone())),
                    _ => {}
                },
                Event::Mouse(m) => match m.kind {
                    MouseEventKind::ScrollUp => {
                        selected = selected.saturating_sub(1);
                    }
                    MouseEventKind::ScrollDown => {
                        if selected + 1 < threads.len() {
                            selected += 1;
                        }
                    }
                    MouseEventKind::Down(MouseButton::Left) => {
                        let size = terminal.size()?;
                        let layout = compute_picker_layout(TerminalSize {
                            width: size.width as usize,
                            height: size.height as usize,
                        });
                        let row0 = m.row as usize;
                        if row0 >= layout.list_y && row0 < layout.list_y + layout.list_h {
                            let idx = top + (row0 - layout.list_y);
                            if idx < threads.len() {
                                selected = idx;
                                return Ok(Some(threads[selected].id.clone()));
                            }
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    })
}

fn run_conversation_tui(
    client: &AppServerClient,
    app: &mut AppState,
    server_events_rx: std::sync::mpsc::Receiver<String>,
) -> Result<()> {
    const MAX_UI_DRAIN_PER_CYCLE: usize = 4096;
    const SERVER_BUDGET_WITH_INPUT: usize = 8;
    const SERVER_BUDGET_IDLE: usize = 256;

    with_terminal(|terminal| {
        let ui_rx = spawn_event_forwarders(server_events_rx);
        let mut deferred_server_lines: VecDeque<String> = VecDeque::new();

        let mut needs_draw = true;
        let mut last_anim_tick = 0u128;

        loop {
            if let Some(perf) = app.perf.as_mut() {
                perf.loop_count = perf.loop_count.saturating_add(1);
            }

            let size = terminal.size()?;
            let size = TerminalSize {
                width: size.width as usize,
                height: size.height as usize,
            };
            let render_started = Instant::now();
            let hidden_user_message_idx = if app.rewind_mode {
                app.rewind_selected_message_idx()
            } else {
                None
            };
            app.ensure_rendered_lines(transcript_content_width(size), hidden_user_message_idx);
            if let Some(perf) = app.perf.as_mut() {
                perf.transcript_render.push(render_started.elapsed());
            }

            let working = app.active_turn_id.is_some();
            let tick = if working { animation_tick() } else { 0 };
            if working {
                if tick != last_anim_tick {
                    needs_draw = true;
                }
            } else if last_anim_tick != 0 {
                needs_draw = true;
            }

            if needs_draw {
                let draw_started = Instant::now();
                terminal.draw(|frame| {
                    render_main_view(frame, app);
                })?;
                if let Some(perf) = app.perf.as_mut() {
                    perf.record_draw(draw_started.elapsed());
                }
                needs_draw = false;
                last_anim_tick = tick;
            }

            let wait_started = Instant::now();
            let next_event = if !deferred_server_lines.is_empty() {
                match ui_rx.try_recv() {
                    Ok(ev) => Some(ev),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => return Ok(()),
                }
            } else if working {
                match ui_rx.recv_timeout(animation_poll_timeout(true)) {
                    Ok(ev) => Some(ev),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => return Ok(()),
                }
            } else {
                match ui_rx.recv() {
                    Ok(ev) => Some(ev),
                    Err(_) => return Ok(()),
                }
            };
            if let Some(perf) = app.perf.as_mut() {
                perf.poll_wait.push(wait_started.elapsed());
            }

            let mut incoming_events = Vec::new();
            if let Some(ev) = next_event {
                incoming_events.push(ev);
            }
            for _ in 0..MAX_UI_DRAIN_PER_CYCLE {
                match ui_rx.try_recv() {
                    Ok(ev) => incoming_events.push(ev),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => return Ok(()),
                }
            }
            let has_terminal_input = incoming_events
                .iter()
                .any(|ev| matches!(ev, UiEvent::Terminal(_)));
            let server_budget = if has_terminal_input {
                SERVER_BUDGET_WITH_INPUT
            } else {
                SERVER_BUDGET_IDLE
            };
            let prioritized_events =
                prioritize_events(incoming_events, &mut deferred_server_lines, server_budget);
            if prioritized_events.is_empty() {
                continue;
            }

            for next_event in prioritized_events {
                let event_started = Instant::now();
                match next_event {
                    UiEvent::ServerLine(line) => {
                        if let Some(perf) = app.perf.as_mut() {
                            perf.notifications = perf.notifications.saturating_add(1);
                        }
                        handle_notification_line(app, &line);
                        needs_draw = true;
                    }
                    UiEvent::Terminal(ev) => match ev {
                        Event::Key(k) => {
                            if let Some(perf) = app.perf.as_mut() {
                                perf.mark_key_kind(k.kind);
                            }
                            if !is_key_press_like(k.kind) {
                                continue;
                            }
                            if let Some(perf) = app.perf.as_mut() {
                                perf.mark_key_event();
                            }
                            if is_perf_toggle_key(k.code, k.modifiers) {
                                if let Some(perf) = app.perf.as_mut() {
                                    perf.toggle_overlay();
                                }
                                needs_draw = true;
                                continue;
                            }
                            let now = Instant::now();
                            app.expire_esc_chord(now);
                            if k.code != KeyCode::Esc {
                                app.reset_esc_chord();
                            }
                            if app.show_help {
                                match (k.code, k.modifiers) {
                                    (code, mods) if is_ctrl_char(code, mods, 'c') => return Ok(()),
                                    (KeyCode::Esc, _) => {
                                        app.show_help = false;
                                    }
                                    (KeyCode::Char('?'), _) => {
                                        app.show_help = false;
                                    }
                                    _ => {}
                                }
                                needs_draw = true;
                                continue;
                            }

                            if let KeyCode::Char(ch) = k.code {
                                if k.modifiers.is_empty() && consume_mobile_mouse_char(app, ch) {
                                    needs_draw = true;
                                    continue;
                                }
                            }

                            match (k.code, k.modifiers) {
                                (code, mods) if is_ctrl_char(code, mods, 'c') => return Ok(()),
                                (KeyCode::Char('y'), KeyModifiers::CONTROL) => {
                                    if let Some(sel) = app.selection {
                                        let msg_bottom = compute_input_layout(app, size).msg_bottom;
                                        let copied = selected_text(
                                            sel,
                                            &app.rendered_lines,
                                            msg_bottom,
                                            app.scroll_top,
                                        );
                                        if !copied.is_empty() {
                                            let _ = try_copy_clipboard(&copied);
                                        }
                                    } else if let Some(text) = last_assistant_message(&app.messages)
                                    {
                                        let backend = try_copy_clipboard(text);
                                        app.set_status(format!(
                                            "copied last assistant message ({})",
                                            clipboard_backend_label(backend)
                                        ));
                                    }
                                }
                                (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
                                    app.selection = None;
                                    app.mouse_drag_mode = MouseDragMode::Undecided;
                                    app.set_status("selection cleared");
                                }
                                (KeyCode::Esc, mods) if mods.is_empty() => {
                                    if app.rewind_mode {
                                        app.exit_rewind_mode_restore();
                                        app.reset_esc_chord();
                                    } else if let Some(turn_id) = app.active_turn_id.clone() {
                                        app.reset_esc_chord();
                                        let params =
                                            params_turn_interrupt(&app.thread_id, &turn_id);
                                        match client.call(
                                            "turn/interrupt",
                                            params,
                                            Duration::from_secs(10),
                                        ) {
                                            Ok(_) => {
                                                app.append_turn_interrupted_marker();
                                                app.set_status("interrupt requested");
                                            }
                                            Err(e) => app.set_status(format!("{e}")),
                                        }
                                    } else if app.register_escape_press(now) {
                                        app.selection = None;
                                        app.mouse_drag_mode = MouseDragMode::Undecided;
                                        if app.input_is_empty() {
                                            app.enter_rewind_mode();
                                            app.align_rewind_scroll_to_selected_prompt(size);
                                        } else {
                                            app.clear_input();
                                        }
                                    } else {
                                        // First Esc press arms the chord; second Esc triggers action.
                                    }
                                }
                                (KeyCode::Home, _) if app.input_is_empty() => {
                                    app.auto_follow_bottom = false;
                                    app.scroll_top = 0;
                                }
                                (KeyCode::End, _) if app.input_is_empty() => {
                                    app.auto_follow_bottom = true;
                                }
                                (KeyCode::Up, _) => {
                                    if app.navigate_input_history_up() {
                                        app.align_rewind_scroll_to_selected_prompt(size);
                                    }
                                }
                                (KeyCode::Down, _) => {
                                    if app.navigate_input_history_down() {
                                        app.align_rewind_scroll_to_selected_prompt(size);
                                    }
                                }
                                (KeyCode::PageUp, _) => {
                                    app.auto_follow_bottom = false;
                                    app.scroll_top = app.scroll_top.saturating_sub(10);
                                }
                                (KeyCode::PageDown, _) => {
                                    app.auto_follow_bottom = false;
                                    app.scroll_top = app.scroll_top.saturating_add(10);
                                }
                                (KeyCode::Enter, mods) if is_newline_enter(mods) => {
                                    app.input_apply_key(k);
                                }
                                (KeyCode::Enter, _) => {
                                    if app.input_is_empty() {
                                        needs_draw = true;
                                        continue;
                                    }

                                    let text = app.input_text();
                                    app.clear_rewind_mode_state();
                                    app.push_input_history(&text);
                                    app.clear_input();
                                    app.selection = None;

                                    if let Some(turn_id) = app.active_turn_id.clone() {
                                        let params =
                                            params_turn_steer(&app.thread_id, &turn_id, &text);
                                        match client.call(
                                            "turn/steer",
                                            params,
                                            Duration::from_secs(10),
                                        ) {
                                            Ok(_) => app.set_status("sent steer"),
                                            Err(e) => app.set_status(format!("{e}")),
                                        }
                                    } else {
                                        let params = params_turn_start(&app.thread_id, &text);
                                        match client.call(
                                            "turn/start",
                                            params,
                                            Duration::from_secs(10),
                                        ) {
                                            Ok(_) => app.set_status("sent turn"),
                                            Err(e) => app.set_status(format!("{e}")),
                                        }
                                    }
                                }
                                (KeyCode::Char('?'), _) => {
                                    if app.input_is_empty() {
                                        app.show_help = true;
                                    } else {
                                        app.input_apply_key(k);
                                    }
                                }
                                (KeyCode::Char('g'), _) if app.input_is_empty() => {
                                    app.auto_follow_bottom = false;
                                    app.scroll_top = 0;
                                }
                                (KeyCode::Char('G'), _) if app.input_is_empty() => {
                                    app.auto_follow_bottom = true;
                                }
                                _ => {
                                    app.input_apply_key(k);
                                }
                            }
                            needs_draw = true;
                        }
                        Event::Mouse(m) => {
                            if let Some(perf) = app.perf.as_mut() {
                                perf.mouse_events = perf.mouse_events.saturating_add(1);
                            }
                            if app.show_help {
                                continue;
                            }

                            let msg_top = MSG_TOP;
                            let msg_bottom = compute_input_layout(app, size).msg_bottom;
                            if msg_bottom < msg_top {
                                continue;
                            }

                            let row1 = m.row as usize + 1;
                            let in_messages = row1 >= msg_top && row1 <= msg_bottom;
                            let norm_x = normalize_selection_x(m.column as usize);
                            let clamped_y = row1.clamp(msg_top, msg_bottom);
                            let mut mouse_changed = false;

                            match m.kind {
                                MouseEventKind::ScrollUp => {
                                    let prev_scroll = app.scroll_top;
                                    let prev_follow = app.auto_follow_bottom;
                                    app.auto_follow_bottom = false;
                                    app.scroll_top = app.scroll_top.saturating_sub(3);
                                    mouse_changed = app.scroll_top != prev_scroll
                                        || app.auto_follow_bottom != prev_follow;
                                }
                                MouseEventKind::ScrollDown => {
                                    let prev_scroll = app.scroll_top;
                                    let prev_follow = app.auto_follow_bottom;
                                    app.auto_follow_bottom = false;
                                    app.scroll_top = app.scroll_top.saturating_add(3);
                                    mouse_changed = app.scroll_top != prev_scroll
                                        || app.auto_follow_bottom != prev_follow;
                                }
                                MouseEventKind::Down(MouseButton::Left) => {
                                    app.mouse_drag_mode = MouseDragMode::Undecided;
                                    app.mouse_drag_last_row = clamped_y;
                                    if in_messages {
                                        app.selection = Some(Selection {
                                            anchor_x: norm_x,
                                            anchor_y: row1,
                                            focus_x: norm_x,
                                            focus_y: row1,
                                            dragging: true,
                                        });
                                        mouse_changed = true;
                                    }
                                }
                                MouseEventKind::Drag(MouseButton::Left) => {
                                    if let Some(sel) = app.selection.as_mut() {
                                        if sel.dragging {
                                            if app.mouse_drag_mode == MouseDragMode::Undecided {
                                                app.mouse_drag_mode = decide_mouse_drag_mode(
                                                    sel.anchor_x,
                                                    sel.anchor_y,
                                                    norm_x,
                                                    clamped_y,
                                                );
                                            }

                                            match app.mouse_drag_mode {
                                                MouseDragMode::Scroll => {
                                                    let prev_scroll = app.scroll_top;
                                                    let prev_follow = app.auto_follow_bottom;
                                                    app.auto_follow_bottom = false;
                                                    if clamped_y > app.mouse_drag_last_row {
                                                        app.scroll_top =
                                                            app.scroll_top.saturating_sub(
                                                                clamped_y - app.mouse_drag_last_row,
                                                            );
                                                    } else if clamped_y < app.mouse_drag_last_row {
                                                        app.scroll_top =
                                                            app.scroll_top.saturating_add(
                                                                app.mouse_drag_last_row - clamped_y,
                                                            );
                                                    }
                                                    app.mouse_drag_last_row = clamped_y;
                                                    mouse_changed = app.scroll_top != prev_scroll
                                                        || app.auto_follow_bottom != prev_follow;
                                                }
                                                MouseDragMode::Select
                                                | MouseDragMode::Undecided => {
                                                    let prev_focus_x = sel.focus_x;
                                                    let prev_focus_y = sel.focus_y;
                                                    sel.focus_x = norm_x;
                                                    sel.focus_y = clamped_y;
                                                    mouse_changed = sel.focus_x != prev_focus_x
                                                        || sel.focus_y != prev_focus_y;
                                                }
                                            }
                                        }
                                    }
                                }
                                MouseEventKind::Up(MouseButton::Left) => {
                                    if let Some(sel) = app.selection.as_mut() {
                                        if sel.dragging {
                                            let prev_focus_x = sel.focus_x;
                                            let prev_focus_y = sel.focus_y;
                                            sel.focus_x = norm_x;
                                            sel.focus_y = clamped_y;
                                            sel.dragging = false;
                                            mouse_changed = sel.focus_x != prev_focus_x
                                                || sel.focus_y != prev_focus_y
                                                || !sel.dragging;

                                            if app.mouse_drag_mode == MouseDragMode::Scroll {
                                                app.selection = None;
                                            } else {
                                                let copied = selected_text(
                                                    *sel,
                                                    &app.rendered_lines,
                                                    msg_bottom,
                                                    app.scroll_top,
                                                );
                                                if !copied.is_empty() {
                                                    let _ = try_copy_clipboard(&copied);
                                                }
                                            }
                                        }
                                    }
                                    app.mouse_drag_mode = MouseDragMode::Undecided;
                                }
                                _ => {}
                            }
                            if mouse_changed {
                                needs_draw = true;
                            }
                        }
                        Event::Paste(pasted) => {
                            if let Some(perf) = app.perf.as_mut() {
                                perf.paste_events = perf.paste_events.saturating_add(1);
                            }
                            if app.show_help {
                                needs_draw = true;
                                continue;
                            }
                            if app.input_is_empty() {
                                if let Some((_, y)) = parse_mobile_mouse_coords(&pasted) {
                                    apply_mobile_mouse_scroll(app, y);
                                    needs_draw = true;
                                    continue;
                                }
                            }
                            let normalized = normalize_pasted_text(&pasted);
                            if !normalized.is_empty() {
                                app.input_insert_text(normalized);
                                needs_draw = true;
                            }
                        }
                        Event::Resize(_, _) => {
                            if let Some(perf) = app.perf.as_mut() {
                                perf.resize_events = perf.resize_events.saturating_add(1);
                            }
                            needs_draw = true;
                        }
                        _ => {}
                    },
                }
                if let Some(perf) = app.perf.as_mut() {
                    perf.event_handle.push(event_started.elapsed());
                }
            }

            if needs_draw {
                let draw_started = Instant::now();
                terminal.draw(|frame| {
                    render_main_view(frame, app);
                })?;
                if let Some(perf) = app.perf.as_mut() {
                    perf.record_draw(draw_started.elapsed());
                }
                needs_draw = false;
                last_anim_tick = if app.active_turn_id.is_some() {
                    animation_tick()
                } else {
                    0
                };
            }
        }
    })
}

fn prioritize_events(
    incoming_events: Vec<UiEvent>,
    deferred_server_lines: &mut VecDeque<String>,
    server_budget: usize,
) -> Vec<UiEvent> {
    let mut prioritized = Vec::with_capacity(incoming_events.len());
    let mut priority_server_lines: Vec<String> = Vec::new();
    for event in incoming_events {
        match event {
            UiEvent::Terminal(ev) => prioritized.push(UiEvent::Terminal(ev)),
            UiEvent::ServerLine(line) => {
                if is_priority_server_line(&line) {
                    priority_server_lines.push(line);
                } else {
                    deferred_server_lines.push_back(line);
                }
            }
        }
    }
    for line in priority_server_lines {
        prioritized.push(UiEvent::ServerLine(line));
    }

    while let Some(idx) = deferred_server_lines
        .iter()
        .position(|line| is_priority_server_line(line))
    {
        if let Some(line) = deferred_server_lines.remove(idx) {
            prioritized.push(UiEvent::ServerLine(line));
        } else {
            break;
        }
    }

    for _ in 0..server_budget {
        let Some(line) = deferred_server_lines.pop_front() else {
            break;
        };
        prioritized.push(UiEvent::ServerLine(line));
    }
    prioritized
}

fn is_priority_server_line(line: &str) -> bool {
    let Ok(parsed) = serde_json::from_str::<Value>(line) else {
        return false;
    };
    let Some(method) = parsed.get("method").and_then(Value::as_str) else {
        return false;
    };
    matches!(method, "turn/completed" | "turn/started" | "error")
}
fn usage() {
    eprintln!(
        "Usage:\n  carlos\n  carlos resume [SESSION_ID]\n\nEnv:\n  CARLOS_METRICS=1  enable perf overlay + exit report (toggle: F8 or Ctrl+P)"
    );
}

fn env_flag_enabled(name: &str) -> bool {
    match env::var(name) {
        Ok(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return true;
            }
            !matches!(
                trimmed.to_ascii_lowercase().as_str(),
                "0" | "false" | "no" | "off"
            )
        }
        Err(_) => false,
    }
}

pub(crate) fn run() -> Result<()> {
    let mut args = env::args();
    let _bin = args.next();

    let cmd = args.next();
    let mut mode_resume = false;
    let mut resume_id: Option<String> = None;

    if let Some(cmd) = cmd {
        if cmd == "resume" {
            mode_resume = true;
            resume_id = args.next();
        } else {
            usage();
            return Ok(());
        }
    }

    let mut client = AppServerClient::start()?;
    initialize_client(&client)?;
    let server_events_rx = client.take_events_rx()?;

    let cwd = env::current_dir()?.to_string_lossy().to_string();

    let (chosen_thread_id, start_resp) = if mode_resume {
        if let Some(rid) = resume_id {
            let resp = client.call(
                "thread/resume",
                params_thread_resume(&rid),
                Duration::from_secs(20),
            )?;
            let thread_id = parse_thread_id_from_start_or_resume(&resp)?;
            (thread_id, resp)
        } else {
            let list_resp = client.call(
                "thread/list",
                params_thread_list(&cwd),
                Duration::from_secs(15),
            )?;
            let list = parse_thread_list(&list_resp)?;
            let picked = pick_thread(&list)?;
            let Some(session_id) = picked else {
                return Ok(());
            };

            let resp = client.call(
                "thread/resume",
                params_thread_resume(&session_id),
                Duration::from_secs(20),
            )?;
            let thread_id = parse_thread_id_from_start_or_resume(&resp)?;
            (thread_id, resp)
        }
    } else {
        let resp = client.call(
            "thread/start",
            params_thread_start(&cwd),
            Duration::from_secs(20),
        )?;
        let thread_id = parse_thread_id_from_start_or_resume(&resp)?;
        (thread_id, resp)
    };

    let mut app = AppState::new(chosen_thread_id);
    if env_flag_enabled("CARLOS_METRICS") {
        app.enable_perf_metrics();
    }
    load_history_from_start_or_resume(&mut app, &start_resp)?;
    app.set_status("ready");

    let out = run_conversation_tui(&client, &mut app, server_events_rx);
    if let Some(report) = app.perf_report() {
        eprintln!("{report}");
    }
    out
}

#[cfg(test)]
#[path = "../tests.rs"]
mod tests;
