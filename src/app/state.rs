use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;
use ratatui_textarea::TextArea;

use super::approval_state::ApprovalState;
pub(super) use super::approval_state::{
    ApprovalChoice, ApprovalRequestKind, PendingApprovalRequest,
};
use super::context_usage::ContextUsage;
use super::input_history_state::InputHistoryState;
use super::input::make_input_area;
use super::models::{Message, MessageKind, RenderedLine, Role, TerminalSize};
use super::perf::PerfMetrics;
#[cfg(test)]
use super::ralph::RalphConfig;
use super::ralph_runtime_state::{QueuedTurnInput, RalphRuntimeState};
use super::render::{compute_input_layout, textarea_input_from_key};
use super::render_cache_state::RenderCacheState;
pub(super) use super::runtime_settings_state::ModelSettingsField;
use super::runtime_settings_state::RuntimeSettingsState;
#[cfg(test)]
pub(super) use super::runtime_settings_state::{DEFAULT_EFFORT_OPTIONS, DEFAULT_SUMMARY_OPTIONS};
use super::selection::{MouseDragMode, RenderedLineSource, Selection};
use super::transcript_render::{
    build_rendered_lines_with_hidden, format_read_summary_with_count, parse_read_summary,
    transcript_content_width,
};
use super::viewport_state::ViewportState;
use super::{RuntimeDefaults, MSG_TOP};
pub(super) struct AppState {
    pub(super) thread_id: String,
    pub(super) active_turn_id: Option<String>,
    pub(super) messages: Vec<Message>,
    pub(super) render_cache: RenderCacheState,
    pub(super) agent_item_to_index: HashMap<String, usize>,
    pub(super) turn_diff_to_index: HashMap<String, usize>,
    pub(super) command_render_overrides: HashMap<String, String>,

    pub(super) input: TextArea<'static>,
    pub(super) input_history: InputHistoryState,
    pub(super) esc_armed_at: Option<Instant>,
    pub(super) status: String,
    pub(super) turn_start_message_idx: Option<usize>,
    pub(super) ralph_runtime: RalphRuntimeState,
    pub(super) runtime: RuntimeSettingsState,
    pub(super) approval: ApprovalState,

    pub(super) viewport: ViewportState,
    pub(super) context_usage: Option<ContextUsage>,
    pub(super) perf: Option<PerfMetrics>,
}

impl AppState {
    pub(super) fn new(thread_id: String) -> Self {
        Self {
            thread_id,
            active_turn_id: None,
            messages: Vec::new(),
            render_cache: RenderCacheState::new(),
            agent_item_to_index: HashMap::new(),
            turn_diff_to_index: HashMap::new(),
            command_render_overrides: HashMap::new(),
            input: make_input_area(),
            input_history: InputHistoryState::new(),
            esc_armed_at: None,
            status: String::new(),
            turn_start_message_idx: None,
            ralph_runtime: RalphRuntimeState::new(),
            runtime: RuntimeSettingsState::new(),
            approval: ApprovalState::new(),
            viewport: ViewportState::new(),
            context_usage: None,
            perf: None,
        }
    }

    pub(super) fn enable_perf_metrics(&mut self) {
        self.perf = Some(PerfMetrics::new());
    }

    pub(super) fn configure_ralph_options(
        &mut self,
        cwd: PathBuf,
        prompt_path_override: Option<String>,
        done_marker_override: Option<String>,
        blocked_marker_override: Option<String>,
    ) {
        self.ralph_runtime.configure_options(
            cwd,
            prompt_path_override,
            done_marker_override,
            blocked_marker_override,
        );
    }

    #[cfg(test)]
    pub(super) fn enable_ralph_mode(&mut self, config: RalphConfig) {
        let status = self.ralph_runtime.enable(config);
        self.set_status(status);
    }

    pub(super) fn disable_ralph_mode(&mut self) {
        let status = self.ralph_runtime.disable();
        self.set_status(status);
    }

    pub(super) fn request_ralph_toggle(&mut self) -> Result<()> {
        let status = self
            .ralph_runtime
            .request_toggle(self.active_turn_id.is_some())?;
        self.set_status(status);
        Ok(())
    }

    pub(super) fn apply_pending_ralph_toggle(&mut self) -> Result<()> {
        if let Some(status) = self.ralph_runtime.apply_pending_toggle()? {
            self.set_status(status);
        }
        Ok(())
    }

    pub(super) fn perf_report(&self) -> Option<String> {
        self.perf.as_ref().map(PerfMetrics::final_report)
    }

    pub(super) fn set_status(&mut self, s: impl Into<String>) {
        self.status = s.into();
    }

    pub(super) fn set_thread_id(&mut self, thread_id: impl Into<String>) {
        self.thread_id = thread_id.into();
    }

    pub(super) fn set_pending_approval(&mut self, approval: PendingApprovalRequest) {
        self.status = format!("approval requested: {}", approval.title);
        self.approval.pending = Some(approval);
    }

    pub(super) fn clear_pending_approval(&mut self) {
        self.approval.pending = None;
    }

    pub(super) fn set_runtime_settings(
        &mut self,
        model: Option<String>,
        effort: Option<String>,
        summary: Option<String>,
    ) {
        self.runtime.set_runtime_settings(model, effort, summary);
    }

    pub(super) fn set_available_models(&mut self, models: Vec<crate::protocol::ModelInfo>) {
        self.runtime.set_available_models(models);
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn queue_runtime_settings(
        &mut self,
        model: Option<String>,
        effort: Option<String>,
        summary: Option<String>,
    ) {
        self.runtime.queue_runtime_settings(model, effort, summary);
    }

    #[cfg(test)]
    pub(super) fn take_pending_runtime_settings(
        &mut self,
    ) -> (Option<String>, Option<String>, Option<String>) {
        self.runtime.take_pending_runtime_settings()
    }

    pub(super) fn next_turn_runtime_settings(
        &self,
    ) -> (Option<String>, Option<String>, Option<String>) {
        self.runtime.next_turn_runtime_settings()
    }

    pub(super) fn mark_runtime_settings_applied(&mut self) {
        self.runtime.mark_runtime_settings_applied();
    }

    pub(super) fn runtime_settings_label(&self) -> String {
        self.runtime.runtime_settings_label()
    }

    pub(super) fn has_runtime_settings(&self) -> bool {
        self.runtime.has_runtime_settings()
    }

    pub(super) fn runtime_settings_pending(&self) -> bool {
        self.runtime.runtime_settings_pending()
    }

    pub(super) fn open_model_settings(&mut self) {
        self.runtime.open_model_settings();
    }

    pub(super) fn close_model_settings(&mut self) {
        self.runtime.close_model_settings();
    }

    pub(super) fn toggle_model_settings(&mut self) {
        if self.runtime.show_model_settings {
            self.close_model_settings();
        } else {
            self.open_model_settings();
        }
    }

    pub(super) fn model_settings_move_field(&mut self, forward: bool) {
        self.runtime.model_settings_move_field(forward);
    }

    pub(super) fn model_settings_cycle_effort(&mut self, step: isize) {
        self.runtime.model_settings_cycle_effort(step);
    }

    pub(super) fn model_settings_cycle_model(&mut self, step: isize) {
        self.runtime.model_settings_cycle_model(step);
    }

    pub(super) fn model_settings_cycle_summary(&mut self, step: isize) {
        self.runtime.model_settings_cycle_summary(step);
    }

    pub(super) fn model_settings_has_model_choices(&self) -> bool {
        self.runtime.model_settings_has_model_choices()
    }

    pub(super) fn model_settings_insert_char(&mut self, ch: char) {
        self.runtime.model_settings_insert_char(ch);
    }

    pub(super) fn model_settings_backspace(&mut self) {
        self.runtime.model_settings_backspace();
    }

    pub(super) fn apply_model_settings(&mut self) -> RuntimeDefaults {
        let defaults = self.runtime.apply_model_settings();
        if self.active_turn_id.is_some() {
            self.set_status("model/effort/summary pending next turn; saved as default");
        } else {
            self.set_status("model/effort/summary set for next turn; saved as default");
        }
        defaults
    }

    pub(super) fn model_settings_model_value(&self) -> &str {
        self.runtime.model_settings_model_value()
    }

    pub(super) fn model_settings_effort_value(&self) -> &str {
        self.runtime.model_settings_effort_value()
    }

    pub(super) fn model_settings_summary_value(&self) -> &str {
        self.runtime.model_settings_summary_value()
    }

    #[cfg(test)]
    pub(super) fn apply_default_reasoning_summary(&mut self, summary: Option<String>) {
        self.runtime.apply_default_reasoning_summary(summary);
    }

    pub(super) fn queue_ralph_continuation(&mut self, text: impl Into<String>) {
        self.ralph_runtime.queue_continuation(text);
    }

    pub(super) fn queue_turn_input(&mut self, text: impl Into<String>) {
        self.queue_turn_input_with_history(text, true);
    }

    pub(super) fn queue_turn_input_with_history(
        &mut self,
        text: impl Into<String>,
        record_input_history: bool,
    ) {
        self.ralph_runtime
            .enqueue_turn_input(text, record_input_history);
    }

    pub(super) fn has_queued_turn_inputs(&self) -> bool {
        self.ralph_runtime.has_queued_turn_inputs()
    }

    pub(super) fn has_pending_ralph_continuation(&self) -> bool {
        self.ralph_runtime.has_pending_continuation()
    }

    pub(super) fn pending_ralph_continuation_wait(&self, now: Instant) -> Option<Duration> {
        self.ralph_runtime.pending_continuation_wait(now)
    }

    pub(super) fn dequeue_turn_input(&mut self, now: Instant) -> Option<QueuedTurnInput> {
        self.ralph_runtime.dequeue_turn_input(now)
    }

    pub(super) fn input_is_empty(&self) -> bool {
        self.input.is_empty()
    }

    pub(super) fn input_text(&self) -> String {
        self.input.lines().join("\n")
    }

    pub(super) fn clear_input(&mut self) {
        self.input = make_input_area();
        self.reset_input_history_navigation();
    }

    pub(super) fn set_input_text(&mut self, text: &str) {
        self.input = make_input_area();
        if !text.is_empty() {
            let _ = self.input.insert_str(text);
        }
    }

    pub(super) fn reset_input_history_navigation(&mut self) {
        self.input_history.reset_navigation();
    }

    pub(super) fn reset_esc_chord(&mut self) {
        self.esc_armed_at = None;
    }

    pub(super) fn expire_esc_chord(&mut self, now: Instant) {
        const ESC_CHORD_WINDOW: Duration = Duration::from_millis(700);
        if let Some(armed_at) = self.esc_armed_at {
            if now.duration_since(armed_at) > ESC_CHORD_WINDOW {
                self.esc_armed_at = None;
            }
        }
    }

    pub(super) fn register_escape_press(&mut self, now: Instant) -> bool {
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

    pub(super) fn enter_rewind_mode(&mut self) {
        if !self.input_history.enter_rewind_mode(self.input_text()) {
            return;
        }
        self.viewport.auto_follow_bottom = false;
        let _ = self.navigate_input_history_up();
    }

    pub(super) fn exit_rewind_mode_restore(&mut self) {
        let Some(draft) = self.input_history.exit_rewind_mode_restore() else {
            return;
        };
        self.viewport.auto_follow_bottom = true;
        self.set_input_text(&draft);
    }

    pub(super) fn clear_rewind_mode_state(&mut self) {
        self.input_history.clear_rewind_mode_state();
        self.viewport.auto_follow_bottom = true;
    }

    pub(super) fn rewind_fork_from_message_idx(&mut self, message_idx: Option<usize>) {
        let Some(idx) = message_idx else {
            return;
        };
        if idx > self.messages.len() {
            return;
        }

        self.messages.truncate(idx);
        self.viewport.selection = None;
        self.viewport.mouse_drag_mode = MouseDragMode::Undecided;
        self.viewport.auto_follow_bottom = true;
        self.viewport.scroll_top = self.viewport.scroll_top.min(self.messages.len());
        self.agent_item_to_index.retain(|_, msg_idx| *msg_idx < idx);
        self.turn_diff_to_index.retain(|_, msg_idx| *msg_idx < idx);
        self.command_render_overrides.clear();
        self.input_history.clear_message_indices_from(idx);
        self.mark_transcript_dirty_from(idx);
    }

    pub(super) fn mark_user_turn_submitted(&mut self) {
        self.ralph_runtime.mark_user_turn_submitted();
    }

    pub(super) fn push_input_history(&mut self, text: &str) {
        self.input_history.push_history(text);
    }

    pub(super) fn record_input_history(&mut self, text: &str, message_idx: Option<usize>) {
        self.input_history.record_history(text, message_idx);
    }

    pub(super) fn navigate_input_history_up(&mut self) -> bool {
        let Some(text) = self.input_history.navigate_up(self.input_text()) else {
            return false;
        };
        self.set_input_text(&text);
        true
    }

    pub(super) fn navigate_input_history_down(&mut self) -> bool {
        let Some(text) = self.input_history.navigate_down() else {
            return false;
        };
        self.set_input_text(&text);
        true
    }

    pub(super) fn rewind_selected_message_idx(&self) -> Option<usize> {
        self.input_history.rewind_selected_message_idx()
    }

    pub(super) fn align_rewind_scroll_to_selected_prompt(&mut self, size: TerminalSize) {
        if !self.rewind_mode() {
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
        self.viewport.scroll_top = target_line.saturating_sub(msg_height.saturating_sub(1));
    }

    pub(super) fn input_apply_key(&mut self, key: crossterm::event::KeyEvent) {
        self.reset_esc_chord();
        if !self.rewind_mode() {
            self.reset_input_history_navigation();
        }
        let _ = self.input.input(textarea_input_from_key(key));
    }

    pub(super) fn input_insert_text(&mut self, text: String) {
        self.reset_esc_chord();
        if !self.rewind_mode() {
            self.reset_input_history_navigation();
        }
        let _ = self.input.insert_str(text);
    }

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

    pub(super) fn append_context_compacted_marker(&mut self) {
        const MARKER: &str = "↻ Context compacted";
        if let Some(last) = self.messages.last() {
            if last.role == Role::System && last.text == MARKER {
                return;
            }
        }
        self.append_message(Role::System, MARKER);
    }

    pub(super) fn append_turn_interrupted_marker(&mut self) {
        const MARKER: &str = "Turn interrupted";
        if let Some(last) = self.messages.last() {
            if last.role == Role::System && last.text == MARKER {
                return;
            }
        }
        self.append_message(Role::System, MARKER);
    }

    pub(super) fn mark_turn_started(&mut self) {
        self.turn_start_message_idx = Some(self.messages.len());
    }

    pub(super) fn maybe_disable_ralph_on_blocked_marker(&mut self) {
        let start_idx = self.turn_start_message_idx.unwrap_or(self.messages.len());
        let Some(outcome) = self
            .ralph_runtime
            .blocked_marker_outcome(&self.messages, start_idx)
        else {
            return;
        };

        if let Some(msg) = outcome.system_message {
            if !self
                .messages
                .last()
                .is_some_and(|last| last.role == Role::System && last.text == msg)
            {
                self.append_message(Role::System, msg);
            }
        }
        if outcome.disable {
            self.disable_ralph_mode();
        }
        if let Some(status) = outcome.status {
            self.set_status(status);
        }
    }

    pub(super) fn handle_ralph_turn_completed(&mut self, interrupted: bool) {
        let start_idx = self
            .turn_start_message_idx
            .take()
            .unwrap_or(self.messages.len());
        let outcome = self
            .ralph_runtime
            .handle_turn_completed(&self.messages, start_idx, interrupted);

        if let Some(msg) = outcome.system_message {
            if !self
                .messages
                .last()
                .is_some_and(|last| last.role == Role::System && last.text == msg)
            {
                self.append_message(Role::System, msg);
            }
        }
        if let Some(text) = outcome.continuation {
            self.queue_ralph_continuation(text);
        }
        if outcome.disable {
            self.disable_ralph_mode();
        }
        if let Some(status) = outcome.status {
            self.set_status(status);
        }
    }

    pub(super) fn ralph_enabled(&self) -> bool {
        self.ralph_runtime.is_enabled()
    }

    pub(super) fn rewind_mode(&self) -> bool {
        self.input_history.rewind_mode()
    }

    #[cfg(test)]
    pub(super) fn queued_turn_inputs_is_empty(&self) -> bool {
        self.ralph_runtime.queued_turn_inputs_is_empty()
    }

    #[cfg(test)]
    pub(super) fn ralph_toggle_pending(&self) -> bool {
        self.ralph_runtime.toggle_pending()
    }

    #[cfg(test)]
    pub(super) fn ralph_pending_continuation_deadline(&self) -> Option<Instant> {
        self.ralph_runtime.pending_continuation_deadline()
    }

    #[cfg(test)]
    pub(super) fn ralph_waiting_for_user(&self) -> bool {
        self.ralph_runtime
            .state()
            .is_some_and(|ralph| ralph.waiting_for_user)
    }

    #[cfg(test)]
    pub(super) fn input_history_message_indices(&self) -> &[Option<usize>] {
        self.input_history.message_indices()
    }

    #[cfg(test)]
    pub(super) fn input_history_len(&self) -> usize {
        self.input_history.history_len()
    }

    #[cfg(test)]
    pub(super) fn set_rewind_selection_for_test(&mut self, index: Option<usize>) {
        self.input_history.set_rewind_selection(index);
    }
}

impl RenderedLineSource for AppState {
    fn len(&self) -> usize {
        self.rendered_line_count()
    }

    fn get(&self, idx: usize) -> Option<&RenderedLine> {
        self.rendered_line_at(idx)
    }
}

fn normalize_reasoning_summary_stream(text: &str) -> String {
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

                while i < text.len() {
                    let rest = &text[i..];
                    if rest.starts_with("**") {
                        break;
                    }
                    let mut chars = rest.chars();
                    let Some(ch) = chars.next() else {
                        break;
                    };
                    if ch.is_whitespace() {
                        i += ch.len_utf8();
                        continue;
                    }
                    break;
                }
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
