use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;
use ratatui_textarea::TextArea;
use serde_json::{json, Value};

use super::context_usage::ContextUsage;
use super::input::make_input_area;
use super::models::{Message, MessageKind, RenderedLine, Role, TerminalSize};
use super::perf::PerfMetrics;
use super::ralph::{detect_turn_markers, load_ralph_config, RalphConfig, RalphState};
use super::render::{
    build_rendered_block_for_message, build_rendered_lines_with_hidden, compute_input_layout,
    count_rendered_block_for_message_cached, format_read_summary_with_count, parse_read_summary,
    textarea_input_from_key, transcript_content_width, RenderCountCache,
};
use super::selection::{MouseDragMode, RenderedLineSource, Selection};
use super::{RuntimeDefaults, MSG_TOP};
use crate::protocol::ModelInfo;

pub(super) const DEFAULT_EFFORT_OPTIONS: [&str; 6] =
    ["none", "minimal", "low", "medium", "high", "xhigh"];
pub(super) const DEFAULT_SUMMARY_OPTIONS: [&str; 4] = ["auto", "concise", "detailed", "none"];
const RALPH_CONTINUATION_DELAY: Duration = Duration::from_millis(700);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ModelSettingsField {
    Model,
    Effort,
    Summary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ApprovalChoice {
    Accept,
    AcceptForSession,
    Decline,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ApprovalRequestKind {
    CommandExecution,
    FileChange,
    Permissions,
    LegacyExecCommand,
    LegacyApplyPatch,
}

#[derive(Debug, Clone)]
pub(super) struct PendingApprovalRequest {
    pub(super) request_id: Value,
    pub(super) method: String,
    pub(super) kind: ApprovalRequestKind,
    pub(super) title: String,
    pub(super) detail_lines: Vec<String>,
    pub(super) requested_permissions: Option<Value>,
    pub(super) can_accept_for_session: bool,
    pub(super) can_decline: bool,
    pub(super) can_cancel: bool,
}

impl PendingApprovalRequest {
    pub(super) fn response_for_choice(&self, choice: ApprovalChoice) -> Option<Value> {
        match self.kind {
            ApprovalRequestKind::CommandExecution => match choice {
                ApprovalChoice::Accept => Some(json!({ "decision": "accept" })),
                ApprovalChoice::AcceptForSession if self.can_accept_for_session => {
                    Some(json!({ "decision": "acceptForSession" }))
                }
                ApprovalChoice::Decline if self.can_decline => {
                    Some(json!({ "decision": "decline" }))
                }
                ApprovalChoice::Cancel if self.can_cancel => Some(json!({ "decision": "cancel" })),
                _ => None,
            },
            ApprovalRequestKind::FileChange => match choice {
                ApprovalChoice::Accept => Some(json!({ "decision": "accept" })),
                ApprovalChoice::AcceptForSession if self.can_accept_for_session => {
                    Some(json!({ "decision": "acceptForSession" }))
                }
                ApprovalChoice::Decline if self.can_decline => {
                    Some(json!({ "decision": "decline" }))
                }
                ApprovalChoice::Cancel if self.can_cancel => Some(json!({ "decision": "cancel" })),
                _ => None,
            },
            ApprovalRequestKind::Permissions => match choice {
                ApprovalChoice::Accept => Some(json!({
                    "permissions": self.requested_permissions.clone().unwrap_or_else(|| json!({}))
                })),
                ApprovalChoice::AcceptForSession if self.can_accept_for_session => Some(json!({
                    "permissions": self.requested_permissions.clone().unwrap_or_else(|| json!({})),
                    "scope": "session"
                })),
                ApprovalChoice::Decline if self.can_decline => Some(json!({
                    "permissions": {}
                })),
                _ => None,
            },
            ApprovalRequestKind::LegacyExecCommand | ApprovalRequestKind::LegacyApplyPatch => {
                match choice {
                    ApprovalChoice::Accept => Some(json!({ "decision": "approved" })),
                    ApprovalChoice::AcceptForSession if self.can_accept_for_session => {
                        Some(json!({ "decision": "approved_for_session" }))
                    }
                    ApprovalChoice::Decline if self.can_decline => {
                        Some(json!({ "decision": "denied" }))
                    }
                    ApprovalChoice::Cancel if self.can_cancel => {
                        Some(json!({ "decision": "abort" }))
                    }
                    _ => None,
                }
            }
        }
    }
}

pub(super) struct AppState {
    pub(super) thread_id: String,
    pub(super) active_turn_id: Option<String>,
    pub(super) messages: Vec<Message>,
    pub(super) rendered_message_blocks: Vec<Option<Vec<RenderedLine>>>,
    pub(super) rendered_block_line_counts: Vec<usize>,
    pub(super) rendered_block_offsets: Vec<usize>,
    pub(super) rendered_total_lines: usize,
    pub(super) rendered_width: usize,
    pub(super) rendered_hidden_user_message_idx: Option<usize>,
    pub(super) transcript_dirty_from: Option<usize>,
    pub(super) agent_item_to_index: HashMap<String, usize>,
    pub(super) turn_diff_to_index: HashMap<String, usize>,
    pub(super) command_render_overrides: HashMap<String, String>,

    pub(super) input: TextArea<'static>,
    pub(super) input_history: Vec<String>,
    pub(super) input_history_message_idx: Vec<Option<usize>>,
    pub(super) input_history_index: Option<usize>,
    pub(super) input_history_draft: Option<String>,
    pub(super) rewind_mode: bool,
    pub(super) rewind_restore_draft: Option<String>,
    pub(super) esc_armed_at: Option<Instant>,
    pub(super) status: String,
    pub(super) turn_start_message_idx: Option<usize>,
    pub(super) queued_turn_inputs: VecDeque<String>,
    pub(super) ralph: Option<RalphState>,
    pub(super) ralph_toggle_pending: bool,
    pub(super) ralph_pending_continuation_deadline: Option<Instant>,
    pub(super) ralph_cwd: PathBuf,
    pub(super) ralph_prompt_path_override: Option<String>,
    pub(super) ralph_done_marker_override: Option<String>,
    pub(super) ralph_blocked_marker_override: Option<String>,
    pub(super) current_model: Option<String>,
    pub(super) current_effort: Option<String>,
    pub(super) current_summary: Option<String>,
    pub(super) pending_model: Option<String>,
    pub(super) pending_effort: Option<String>,
    pub(super) pending_summary: Option<String>,
    pub(super) show_model_settings: bool,
    pub(super) model_settings_field: ModelSettingsField,
    pub(super) model_settings_model_input: String,
    pub(super) model_settings_model_index: usize,
    pub(super) model_settings_effort_options: Vec<String>,
    pub(super) model_settings_effort_index: usize,
    pub(super) model_settings_summary_options: Vec<String>,
    pub(super) model_settings_summary_index: usize,
    pub(super) available_models: Vec<ModelInfo>,
    pub(super) pending_approval: Option<PendingApprovalRequest>,

    pub(super) scroll_top: usize,
    pub(super) auto_follow_bottom: bool,
    pub(super) selection: Option<Selection>,
    pub(super) mouse_drag_mode: MouseDragMode,
    pub(super) mouse_drag_last_row: usize,
    pub(super) mobile_mouse_buffer: String,
    pub(super) mobile_mouse_last_y: Option<usize>,
    pub(super) mobile_plain_pending_coords: bool,
    pub(super) mobile_plain_suppress_coords: bool,
    pub(super) mobile_plain_last_direction: i8,
    pub(super) mobile_plain_new_gesture: bool,
    pub(super) show_help: bool,
    pub(super) scroll_inverted: bool,
    pub(super) context_usage: Option<ContextUsage>,
    pub(super) perf: Option<PerfMetrics>,
}

impl AppState {
    pub(super) fn new(thread_id: String) -> Self {
        Self {
            thread_id,
            active_turn_id: None,
            messages: Vec::new(),
            rendered_message_blocks: Vec::new(),
            rendered_block_line_counts: Vec::new(),
            rendered_block_offsets: Vec::new(),
            rendered_total_lines: 0,
            rendered_width: 0,
            rendered_hidden_user_message_idx: None,
            transcript_dirty_from: Some(0),
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
            turn_start_message_idx: None,
            queued_turn_inputs: VecDeque::new(),
            ralph: None,
            ralph_toggle_pending: false,
            ralph_pending_continuation_deadline: None,
            ralph_cwd: PathBuf::new(),
            ralph_prompt_path_override: None,
            ralph_done_marker_override: None,
            ralph_blocked_marker_override: None,
            current_model: None,
            current_effort: None,
            current_summary: None,
            pending_model: None,
            pending_effort: None,
            pending_summary: None,
            show_model_settings: false,
            model_settings_field: ModelSettingsField::Model,
            model_settings_model_input: String::new(),
            model_settings_model_index: 0,
            model_settings_effort_options: Vec::new(),
            model_settings_effort_index: 3,
            model_settings_summary_options: DEFAULT_SUMMARY_OPTIONS
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            model_settings_summary_index: 0,
            available_models: Vec::new(),
            pending_approval: None,
            scroll_top: 0,
            auto_follow_bottom: true,
            selection: None,
            mouse_drag_mode: MouseDragMode::Undecided,
            mouse_drag_last_row: 0,
            mobile_mouse_buffer: String::new(),
            mobile_mouse_last_y: None,
            mobile_plain_pending_coords: false,
            mobile_plain_suppress_coords: false,
            mobile_plain_last_direction: 0,
            mobile_plain_new_gesture: false,
            show_help: false,
            scroll_inverted: false,
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
        self.ralph_cwd = cwd;
        self.ralph_prompt_path_override = prompt_path_override;
        self.ralph_done_marker_override = done_marker_override;
        self.ralph_blocked_marker_override = blocked_marker_override;
    }

    pub(super) fn enable_ralph_mode(&mut self, config: RalphConfig) {
        let prompt_path = config.prompt_path.display().to_string();
        self.ralph = Some(RalphState::new(config));
        self.ralph_toggle_pending = false;
        self.set_status(format!("ralph on ({prompt_path})"));
    }

    pub(super) fn disable_ralph_mode(&mut self) {
        self.ralph = None;
        self.ralph_toggle_pending = false;
        self.ralph_pending_continuation_deadline = None;
        self.queued_turn_inputs.clear();
        self.set_status("ralph off");
    }

    pub(super) fn queue_ralph_initial_prompt(&mut self) {
        let Some(ralph) = self.ralph.as_mut() else {
            return;
        };
        if ralph.primed || ralph.completed {
            return;
        }
        ralph.primed = true;
        let prompt = ralph.config.base_prompt.clone();
        self.enqueue_turn_input(prompt);
    }

    pub(super) fn toggle_ralph_mode_now(&mut self) -> Result<()> {
        if self.ralph.is_some() {
            self.disable_ralph_mode();
            return Ok(());
        }

        let cfg = load_ralph_config(
            &self.ralph_cwd,
            self.ralph_prompt_path_override.as_deref(),
            self.ralph_done_marker_override.as_deref(),
            self.ralph_blocked_marker_override.as_deref(),
        )?;
        self.enable_ralph_mode(cfg);
        self.queue_ralph_initial_prompt();
        self.set_status("ralph on");
        Ok(())
    }

    pub(super) fn request_ralph_toggle(&mut self) -> Result<()> {
        if self.active_turn_id.is_some() {
            self.ralph_toggle_pending = !self.ralph_toggle_pending;
            if self.ralph_toggle_pending {
                self.set_status("ralph toggle pending");
            } else {
                self.set_status("ralph toggle canceled");
            }
            return Ok(());
        }
        self.toggle_ralph_mode_now()
    }

    pub(super) fn apply_pending_ralph_toggle(&mut self) -> Result<()> {
        if !self.ralph_toggle_pending {
            return Ok(());
        }
        self.ralph_toggle_pending = false;
        self.toggle_ralph_mode_now()
    }

    pub(super) fn perf_report(&self) -> Option<String> {
        self.perf.as_ref().map(PerfMetrics::final_report)
    }

    pub(super) fn set_status(&mut self, s: impl Into<String>) {
        self.status = s.into();
    }

    pub(super) fn set_pending_approval(&mut self, approval: PendingApprovalRequest) {
        self.status = format!("approval requested: {}", approval.title);
        self.pending_approval = Some(approval);
    }

    pub(super) fn clear_pending_approval(&mut self) {
        self.pending_approval = None;
    }

    pub(super) fn set_runtime_settings(
        &mut self,
        model: Option<String>,
        effort: Option<String>,
        summary: Option<String>,
    ) {
        self.current_model = model.and_then(normalize_non_empty);
        self.current_effort = effort.and_then(normalize_non_empty);
        self.current_summary = summary.and_then(normalize_non_empty);
    }

    pub(super) fn set_available_models(&mut self, mut models: Vec<ModelInfo>) {
        models.sort_by_key(|m| !m.is_default);
        self.available_models = models;
    }

    pub(super) fn queue_runtime_settings(
        &mut self,
        model: Option<String>,
        effort: Option<String>,
        summary: Option<String>,
    ) {
        self.pending_model = model.and_then(normalize_non_empty);
        self.pending_effort = effort.and_then(normalize_non_empty);
        self.pending_summary = summary.and_then(normalize_non_empty);
    }

    pub(super) fn take_pending_runtime_settings(
        &mut self,
    ) -> (Option<String>, Option<String>, Option<String>) {
        (
            self.pending_model.clone(),
            self.pending_effort.clone(),
            self.pending_summary.clone(),
        )
    }

    pub(super) fn mark_runtime_settings_applied(&mut self) {
        if self.pending_model.is_some()
            || self.pending_effort.is_some()
            || self.pending_summary.is_some()
        {
            self.current_model = self.pending_model.clone();
            self.current_effort = self.pending_effort.clone();
            self.current_summary = self.pending_summary.clone();
            self.pending_model = None;
            self.pending_effort = None;
            self.pending_summary = None;
        }
    }

    pub(super) fn runtime_settings_label(&self) -> String {
        let shown_model = self
            .pending_model
            .as_deref()
            .or(self.current_model.as_deref())
            .unwrap_or("model?");
        let shown_effort = self
            .pending_effort
            .as_deref()
            .or(self.current_effort.as_deref())
            .unwrap_or("effort?");
        let shown_summary = self
            .pending_summary
            .as_deref()
            .or(self.current_summary.as_deref())
            .unwrap_or("summary?");
        let mut out = format!("{shown_model}/{shown_effort}/{shown_summary}");

        let pending_differs = self.pending_model.as_deref() != self.current_model.as_deref()
            || self.pending_effort.as_deref() != self.current_effort.as_deref()
            || self.pending_summary.as_deref() != self.current_summary.as_deref();
        if self.runtime_settings_pending() && pending_differs {
            out.push('*');
        }

        out
    }

    pub(super) fn has_runtime_settings(&self) -> bool {
        self.current_model.is_some()
            || self.current_effort.is_some()
            || self.current_summary.is_some()
    }

    pub(super) fn runtime_settings_pending(&self) -> bool {
        self.pending_model.is_some()
            || self.pending_effort.is_some()
            || self.pending_summary.is_some()
    }

    pub(super) fn open_model_settings(&mut self) {
        self.show_model_settings = true;
        self.model_settings_field = ModelSettingsField::Model;
        let preferred_model = self
            .pending_model
            .as_deref()
            .or(self.current_model.as_deref())
            .unwrap_or("");
        self.model_settings_model_index = if self.available_models.is_empty() {
            0
        } else {
            self.available_models
                .iter()
                .position(|m| m.model == preferred_model)
                .unwrap_or(0)
        };
        self.model_settings_model_input = self
            .available_models
            .get(self.model_settings_model_index)
            .map(|m| m.model.clone())
            .unwrap_or_else(|| preferred_model.to_string());
        self.refresh_model_settings_efforts();
        let preferred_summary = self
            .pending_summary
            .as_deref()
            .or(self.current_summary.as_deref())
            .unwrap_or("auto");
        self.model_settings_summary_index = self
            .model_settings_summary_options
            .iter()
            .position(|option| option == preferred_summary)
            .unwrap_or(0);
    }

    pub(super) fn close_model_settings(&mut self) {
        self.show_model_settings = false;
    }

    pub(super) fn toggle_model_settings(&mut self) {
        if self.show_model_settings {
            self.close_model_settings();
        } else {
            self.open_model_settings();
        }
    }

    pub(super) fn model_settings_move_field(&mut self, forward: bool) {
        self.model_settings_field = match (self.model_settings_field, forward) {
            (ModelSettingsField::Model, true) => ModelSettingsField::Effort,
            (ModelSettingsField::Effort, true) => ModelSettingsField::Summary,
            (ModelSettingsField::Summary, true) => ModelSettingsField::Model,
            (ModelSettingsField::Model, false) => ModelSettingsField::Summary,
            (ModelSettingsField::Effort, false) => ModelSettingsField::Model,
            (ModelSettingsField::Summary, false) => ModelSettingsField::Effort,
        };
    }

    pub(super) fn model_settings_cycle_effort(&mut self, step: isize) {
        if self.model_settings_effort_options.is_empty() {
            self.model_settings_effort_options = DEFAULT_EFFORT_OPTIONS
                .iter()
                .map(|s| (*s).to_string())
                .collect();
        }
        let len = self.model_settings_effort_options.len() as isize;
        let cur = self.model_settings_effort_index as isize;
        let next = (cur + step).rem_euclid(len);
        self.model_settings_effort_index = next as usize;
    }

    pub(super) fn model_settings_cycle_model(&mut self, step: isize) {
        if self.available_models.is_empty() {
            return;
        }
        let len = self.available_models.len() as isize;
        let cur = self.model_settings_model_index as isize;
        let next = (cur + step).rem_euclid(len);
        self.model_settings_model_index = next as usize;
        if let Some(model) = self.available_models.get(self.model_settings_model_index) {
            self.model_settings_model_input = model.model.clone();
        }
        self.refresh_model_settings_efforts();
    }

    pub(super) fn model_settings_cycle_summary(&mut self, step: isize) {
        let len = self.model_settings_summary_options.len() as isize;
        let cur = self.model_settings_summary_index as isize;
        let next = (cur + step).rem_euclid(len);
        self.model_settings_summary_index = next as usize;
    }

    pub(super) fn model_settings_has_model_choices(&self) -> bool {
        !self.available_models.is_empty()
    }

    pub(super) fn model_settings_insert_char(&mut self, ch: char) {
        self.model_settings_model_input.push(ch);
    }

    pub(super) fn model_settings_backspace(&mut self) {
        self.model_settings_model_input.pop();
    }

    pub(super) fn apply_model_settings(&mut self) -> RuntimeDefaults {
        let model = normalize_non_empty(self.model_settings_model_value().to_string());
        let effort = normalize_non_empty(self.model_settings_effort_value().to_string());
        let summary = normalize_non_empty(self.model_settings_summary_value().to_string());
        let defaults = RuntimeDefaults {
            model: model.clone(),
            effort: effort.clone(),
            summary: summary.clone(),
        };
        self.queue_runtime_settings(model, effort, summary);
        self.show_model_settings = false;
        if self.active_turn_id.is_some() {
            self.set_status("model/effort/summary pending next turn; saved as default");
        } else {
            self.set_status("model/effort/summary set for next turn; saved as default");
        }
        defaults
    }

    pub(super) fn model_settings_model_value(&self) -> &str {
        if let Some(model) = self.available_models.get(self.model_settings_model_index) {
            return model.model.as_str();
        }
        self.model_settings_model_input.as_str()
    }

    pub(super) fn model_settings_effort_value(&self) -> &str {
        self.model_settings_effort_options
            .get(self.model_settings_effort_index)
            .map(String::as_str)
            .unwrap_or("medium")
    }

    pub(super) fn model_settings_summary_value(&self) -> &str {
        self.model_settings_summary_options
            .get(self.model_settings_summary_index)
            .map(String::as_str)
            .unwrap_or("auto")
    }

    pub(super) fn apply_default_reasoning_summary(&mut self, summary: Option<String>) {
        if self.current_summary.is_none() && self.pending_summary.is_none() {
            self.pending_summary = summary.and_then(normalize_non_empty);
        }
    }

    pub(super) fn enqueue_turn_input(&mut self, text: impl Into<String>) {
        let text = text.into();
        if text.trim().is_empty() {
            return;
        }
        self.queued_turn_inputs.push_back(text);
    }

    pub(super) fn queue_ralph_continuation(&mut self, text: impl Into<String>) {
        let text = text.into();
        if text.trim().is_empty() {
            return;
        }
        self.queued_turn_inputs.push_back(text);
        self.ralph_pending_continuation_deadline = Some(Instant::now() + RALPH_CONTINUATION_DELAY);
    }

    pub(super) fn has_pending_ralph_continuation(&self) -> bool {
        self.ralph_pending_continuation_deadline.is_some() && !self.queued_turn_inputs.is_empty()
    }

    pub(super) fn pending_ralph_continuation_wait(&self, now: Instant) -> Option<Duration> {
        self.ralph_pending_continuation_deadline
            .map(|deadline| deadline.saturating_duration_since(now))
            .filter(|wait| !wait.is_zero())
    }

    pub(super) fn dequeue_turn_input(&mut self, now: Instant) -> Option<String> {
        if let Some(deadline) = self.ralph_pending_continuation_deadline {
            if now < deadline {
                return None;
            }
            self.ralph_pending_continuation_deadline = None;
        }
        self.queued_turn_inputs.pop_front()
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
        self.input_history_index = None;
        self.input_history_draft = None;
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
        if self.rewind_mode {
            return;
        }
        self.rewind_mode = true;
        self.auto_follow_bottom = false;
        self.rewind_restore_draft = Some(self.input_text());
        self.reset_input_history_navigation();
        let _ = self.navigate_input_history_up();
    }

    pub(super) fn exit_rewind_mode_restore(&mut self) {
        if !self.rewind_mode {
            return;
        }
        let draft = self.rewind_restore_draft.take().unwrap_or_default();
        self.rewind_mode = false;
        self.auto_follow_bottom = true;
        self.set_input_text(&draft);
        self.reset_input_history_navigation();
    }

    pub(super) fn clear_rewind_mode_state(&mut self) {
        self.rewind_mode = false;
        self.auto_follow_bottom = true;
        self.rewind_restore_draft = None;
    }

    pub(super) fn rewind_fork_from_message_idx(&mut self, message_idx: Option<usize>) {
        let Some(idx) = message_idx else {
            return;
        };
        if idx > self.messages.len() {
            return;
        }

        self.messages.truncate(idx);
        self.selection = None;
        self.mouse_drag_mode = MouseDragMode::Undecided;
        self.auto_follow_bottom = true;
        self.scroll_top = self.scroll_top.min(self.messages.len());
        self.agent_item_to_index.retain(|_, msg_idx| *msg_idx < idx);
        self.turn_diff_to_index.retain(|_, msg_idx| *msg_idx < idx);
        self.command_render_overrides.clear();
        for msg_idx in &mut self.input_history_message_idx {
            if msg_idx.is_some_and(|v| v >= idx) {
                *msg_idx = None;
            }
        }
        self.mark_transcript_dirty_from(idx);
    }

    pub(super) fn mark_user_turn_submitted(&mut self) {
        if let Some(ralph) = self.ralph.as_mut() {
            ralph.waiting_for_user = false;
        }
    }

    pub(super) fn push_input_history(&mut self, text: &str) {
        self.record_input_history(text, None);
    }

    pub(super) fn record_input_history(&mut self, text: &str, message_idx: Option<usize>) {
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

    pub(super) fn navigate_input_history_up(&mut self) -> bool {
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

    pub(super) fn navigate_input_history_down(&mut self) -> bool {
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

    pub(super) fn rewind_selected_message_idx(&self) -> Option<usize> {
        let idx = self.input_history_index?;
        self.input_history_message_idx.get(idx).and_then(|v| *v)
    }

    pub(super) fn align_rewind_scroll_to_selected_prompt(&mut self, size: TerminalSize) {
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

    pub(super) fn input_apply_key(&mut self, key: crossterm::event::KeyEvent) {
        self.reset_esc_chord();
        if !self.rewind_mode {
            self.reset_input_history_navigation();
        }
        let _ = self.input.input(textarea_input_from_key(key));
    }

    pub(super) fn input_insert_text(&mut self, text: String) {
        self.reset_esc_chord();
        if !self.rewind_mode {
            self.reset_input_history_navigation();
        }
        let _ = self.input.insert_str(text);
    }

    pub(super) fn mark_transcript_dirty(&mut self) {
        self.mark_transcript_dirty_from(0);
    }

    pub(super) fn mark_transcript_dirty_from(&mut self, idx: usize) {
        let idx = idx.min(self.messages.len());
        self.transcript_dirty_from = Some(match self.transcript_dirty_from {
            Some(current) => current.min(idx),
            None => idx,
        });
    }

    pub(super) fn sync_auto_follow_bottom(&mut self, max_scroll: usize) {
        if self.scroll_top >= max_scroll {
            self.scroll_top = max_scroll;
            self.auto_follow_bottom = true;
        } else {
            self.auto_follow_bottom = false;
        }
    }

    pub(super) fn ensure_rendered_lines(
        &mut self,
        width: usize,
        hidden_user_message_idx: Option<usize>,
    ) {
        let rebuild_from = if self.rendered_width != width
            || self.rendered_hidden_user_message_idx != hidden_user_message_idx
        {
            Some(0)
        } else {
            self.transcript_dirty_from
        };

        let Some(dirty_from) = rebuild_from else {
            return;
        };

        if dirty_from == 0 {
            self.rendered_message_blocks.clear();
            self.rendered_block_line_counts.clear();
            self.rendered_block_offsets.clear();
            self.rendered_total_lines = 0;
        } else {
            let start_offset = self
                .rendered_block_offsets
                .get(dirty_from)
                .copied()
                .unwrap_or(self.rendered_total_lines);
            self.rendered_message_blocks.truncate(dirty_from);
            self.rendered_block_line_counts.truncate(dirty_from);
            self.rendered_block_offsets.truncate(dirty_from);
            self.rendered_total_lines = start_offset;
        }

        let mut previous_visible_idx = if dirty_from == 0 {
            None
        } else {
            self.find_previous_visible_message_idx(dirty_from, hidden_user_message_idx)
        };
        let mut count_cache = RenderCountCache::new();

        for idx in dirty_from..self.messages.len() {
            self.rendered_block_offsets.push(self.rendered_total_lines);

            let hidden =
                hidden_user_message_idx == Some(idx) && self.messages[idx].role == Role::User;
            if hidden {
                self.rendered_message_blocks.push(None);
                self.rendered_block_line_counts.push(0);
                continue;
            }

            let msg = &self.messages[idx];
            if msg.text.trim().is_empty() {
                self.rendered_message_blocks.push(None);
                self.rendered_block_line_counts.push(0);
                continue;
            }

            let previous_visible =
                previous_visible_idx.and_then(|prev_idx| self.messages.get(prev_idx));
            let line_count = count_rendered_block_for_message_cached(
                &mut count_cache,
                previous_visible,
                msg,
                width,
            );
            self.rendered_block_line_counts.push(line_count);
            if line_count == 0 {
                self.rendered_message_blocks.push(None);
                continue;
            }
            self.rendered_total_lines += line_count;
            self.rendered_message_blocks.push(None);
            previous_visible_idx = Some(idx);
        }

        self.rendered_width = width;
        self.rendered_hidden_user_message_idx = hidden_user_message_idx;
        self.transcript_dirty_from = None;
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

    pub(super) fn put_agent_item_mapping(&mut self, item_id: &str, idx: usize) {
        self.agent_item_to_index.insert(item_id.to_string(), idx);
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
        if let Some(idx) = self.agent_item_to_index.get(call_id).copied() {
            if let Some(msg) = self.messages.get_mut(idx) {
                msg.role = Role::ToolCall;
                msg.kind = MessageKind::Plain;
                msg.file_path = None;
                msg.text = summary;
            }
            let dirty_from = self.coalesce_successive_read_summary_at(idx).unwrap_or(idx);
            self.mark_transcript_dirty_from(dirty_from);
        }
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

    fn find_previous_visible_message_idx(
        &self,
        start_idx: usize,
        hidden_user_message_idx: Option<usize>,
    ) -> Option<usize> {
        let mut idx = start_idx;
        while idx > 0 {
            idx -= 1;
            let msg = self.messages.get(idx)?;
            if hidden_user_message_idx == Some(idx) && msg.role == Role::User {
                continue;
            }
            if !msg.text.trim().is_empty() {
                return Some(idx);
            }
        }
        None
    }

    pub(super) fn rendered_line_count(&self) -> usize {
        self.rendered_total_lines
    }

    pub(super) fn rendered_line_at(&self, idx: usize) -> Option<&RenderedLine> {
        if idx >= self.rendered_total_lines {
            return None;
        }
        let pos = self
            .rendered_block_offsets
            .partition_point(|offset| *offset <= idx);
        let block_idx = pos.checked_sub(1)?;
        let block_start = *self.rendered_block_offsets.get(block_idx)?;
        let block = self.rendered_message_blocks.get(block_idx)?.as_ref()?;
        block.get(idx.saturating_sub(block_start))
    }

    pub(super) fn ensure_rendered_range_materialized(&mut self, start_idx: usize, end_idx: usize) {
        if start_idx > end_idx || self.rendered_total_lines == 0 {
            return;
        }

        let start_block = self
            .rendered_block_offsets
            .partition_point(|offset| *offset <= start_idx)
            .saturating_sub(1);
        let end_block = self
            .rendered_block_offsets
            .partition_point(|offset| *offset <= end_idx)
            .saturating_sub(1);

        for block_idx in start_block..=end_block {
            self.ensure_rendered_block_materialized(block_idx);
        }
    }

    fn ensure_rendered_block_materialized(&mut self, block_idx: usize) {
        if self
            .rendered_message_blocks
            .get(block_idx)
            .and_then(Option::as_ref)
            .is_some()
        {
            return;
        }
        if self
            .rendered_block_line_counts
            .get(block_idx)
            .copied()
            .unwrap_or(0)
            == 0
        {
            return;
        }

        let Some(msg) = self.messages.get(block_idx) else {
            return;
        };
        let previous_visible = self
            .find_previous_visible_message_idx(block_idx, self.rendered_hidden_user_message_idx)
            .and_then(|prev_idx| self.messages.get(prev_idx));
        let block = build_rendered_block_for_message(previous_visible, msg, self.rendered_width);
        debug_assert_eq!(
            block.len(),
            self.rendered_block_line_counts
                .get(block_idx)
                .copied()
                .unwrap_or(0)
        );
        if let Some(slot) = self.rendered_message_blocks.get_mut(block_idx) {
            *slot = Some(block);
        }
    }

    #[cfg(test)]
    pub(super) fn snapshot_rendered_lines(&mut self) -> Vec<RenderedLine> {
        if self.rendered_total_lines > 0 {
            self.ensure_rendered_range_materialized(0, self.rendered_total_lines - 1);
        }
        let mut out = Vec::with_capacity(self.rendered_total_lines);
        for block in &self.rendered_message_blocks {
            if let Some(block) = block {
                out.extend(block.iter().cloned());
            }
        }
        out
    }

    pub(super) fn selected_text(&mut self, selection: Selection) -> String {
        let start_idx = selection.anchor_line_idx.min(selection.focus_line_idx);
        let end_idx = selection.anchor_line_idx.max(selection.focus_line_idx);
        if self.rendered_total_lines > 0 {
            self.ensure_rendered_range_materialized(
                start_idx.min(self.rendered_total_lines.saturating_sub(1)),
                end_idx.min(self.rendered_total_lines.saturating_sub(1)),
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
        let Some(ralph) = self.ralph.as_ref() else {
            return;
        };
        let start_idx = self.turn_start_message_idx.unwrap_or(self.messages.len());
        let blocked_marker = ralph.config.blocked_marker.clone();
        let markers = detect_turn_markers(&self.messages, start_idx, "", &blocked_marker);
        if !markers.blocked {
            return;
        }

        const BLOCKED_MSG: &str = "Ralph blocked: waiting for input";
        if !self
            .messages
            .last()
            .is_some_and(|last| last.role == Role::System && last.text == BLOCKED_MSG)
        {
            self.append_message(Role::System, BLOCKED_MSG);
        }
        self.disable_ralph_mode();
        self.set_status("ralph blocked");
    }

    pub(super) fn handle_ralph_turn_completed(&mut self, interrupted: bool) {
        let start_idx = self
            .turn_start_message_idx
            .take()
            .unwrap_or(self.messages.len());
        let mut next_status = None;
        let mut next_message = None;
        let mut continuation = None;
        let mut disable_ralph_mode = false;

        if let Some(ralph) = self.ralph.as_mut() {
            let markers = detect_turn_markers(
                &self.messages,
                start_idx,
                &ralph.config.done_marker,
                &ralph.config.blocked_marker,
            );
            if markers.completed {
                ralph.completed = true;
                ralph.waiting_for_user = false;
                next_message = Some("Ralph complete".to_string());
                next_status = Some("ralph complete".to_string());
                disable_ralph_mode = true;
            } else if markers.blocked {
                ralph.waiting_for_user = false;
                next_message = Some("Ralph blocked: waiting for input".to_string());
                next_status = Some("ralph blocked".to_string());
                disable_ralph_mode = true;
            } else if interrupted {
                ralph.waiting_for_user = false;
            } else if !ralph.completed {
                ralph.waiting_for_user = false;
                continuation = Some(ralph.config.continuation_prompt.clone());
                next_status = Some("ralph continuing".to_string());
            }
        }

        if let Some(msg) = next_message {
            if !self
                .messages
                .last()
                .is_some_and(|last| last.role == Role::System && last.text == msg)
            {
                self.append_message(Role::System, msg);
            }
        }
        if let Some(text) = continuation {
            self.queue_ralph_continuation(text);
        }
        if disable_ralph_mode {
            self.disable_ralph_mode();
        }
        if let Some(status) = next_status {
            self.set_status(status);
        }
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

fn normalize_non_empty(s: String) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
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

fn effort_index(value: &str) -> usize {
    DEFAULT_EFFORT_OPTIONS
        .iter()
        .position(|v| v.eq_ignore_ascii_case(value))
        .unwrap_or(3)
}

impl AppState {
    fn refresh_model_settings_efforts(&mut self) {
        let requested = self
            .pending_effort
            .as_deref()
            .or(self.current_effort.as_deref())
            .unwrap_or("medium")
            .to_string();

        let (options, default_effort) =
            if let Some(model) = self.available_models.get(self.model_settings_model_index) {
                let options = if model.supported_efforts.is_empty() {
                    DEFAULT_EFFORT_OPTIONS
                        .iter()
                        .map(|s| (*s).to_string())
                        .collect::<Vec<_>>()
                } else {
                    model.supported_efforts.clone()
                };
                (options, model.default_effort.clone())
            } else {
                (
                    DEFAULT_EFFORT_OPTIONS
                        .iter()
                        .map(|s| (*s).to_string())
                        .collect::<Vec<_>>(),
                    None,
                )
            };

        self.model_settings_effort_options = options;
        self.model_settings_effort_index = self
            .model_settings_effort_options
            .iter()
            .position(|e| e.eq_ignore_ascii_case(&requested))
            .or_else(|| {
                default_effort.as_deref().and_then(|d| {
                    self.model_settings_effort_options
                        .iter()
                        .position(|e| e.eq_ignore_ascii_case(d))
                })
            })
            .unwrap_or_else(|| {
                effort_index("medium")
                    .min(self.model_settings_effort_options.len().saturating_sub(1))
            });
    }
}
