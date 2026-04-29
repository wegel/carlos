//! Application state: transcript, input, display mode, and sub-state aggregation.

// --- Imports ---

use std::collections::HashMap;
use std::path::PathBuf;
#[cfg(feature = "dictation")]
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::Result;
use ratatui_textarea::TextArea;

use super::approval_state::ApprovalState;
pub(super) use super::approval_state::{
    ApprovalChoice, ApprovalRequestKind, PendingApprovalRequest,
};
use super::context_usage::ContextUsage;
use super::dictation_state::DictationRuntimeState;
use super::input::make_input_area;
use super::input_history_state::InputHistoryState;
use super::models::{Message, Role};
use super::perf::PerfMetrics;
#[cfg(test)]
use super::ralph::RalphConfig;
use super::ralph_runtime_state::{QueuedTurnInput, RalphRuntimeState};
use super::render_cache_state::RenderCacheState;
pub(super) use super::runtime_settings_state::ModelSettingsField;
use super::runtime_settings_state::RuntimeSettingsState;
#[cfg(test)]
pub(super) use super::runtime_settings_state::{DEFAULT_EFFORT_OPTIONS, DEFAULT_SUMMARY_OPTIONS};
use super::viewport_state::ViewportState;
#[cfg(feature = "dictation")]
use crate::dictation::capture::DictationCaptureSession;
#[cfg(feature = "dictation")]
use crate::dictation::events::DictationEvent;
#[cfg(feature = "dictation")]
use crate::dictation::worker::{DictationCancelToken, DictationWorker};

// --- Types ---

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
    pub(super) dictation: DictationRuntimeState,
    #[cfg(feature = "dictation")]
    pub(super) dictation_capture: Option<DictationCaptureSession>,
    #[cfg(feature = "dictation")]
    pub(super) dictation_events_tx: Option<mpsc::Sender<DictationEvent>>,
    #[cfg(feature = "dictation")]
    pub(super) dictation_worker: Option<DictationWorker>,
    #[cfg(feature = "dictation")]
    pub(super) dictation_cancel: Option<DictationCancelToken>,
    #[cfg(feature = "dictation")]
    pub(super) dictation_request_id: Option<u64>,
    #[cfg(feature = "dictation")]
    pub(super) next_dictation_request_id: u64,
    #[cfg(feature = "dictation")]
    pub(super) last_dictation_audio: Option<Vec<f32>>,
    pub(super) runtime: RuntimeSettingsState,
    pub(super) approval: ApprovalState,

    pub(super) viewport: ViewportState,
    pub(super) context_usage: Option<ContextUsage>,
    pub(super) perf: Option<PerfMetrics>,
}

// --- Construction ---

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
            dictation: DictationRuntimeState::disabled("dictation feature is not configured"),
            #[cfg(feature = "dictation")]
            dictation_capture: None,
            #[cfg(feature = "dictation")]
            dictation_events_tx: None,
            #[cfg(feature = "dictation")]
            dictation_worker: None,
            #[cfg(feature = "dictation")]
            dictation_cancel: None,
            #[cfg(feature = "dictation")]
            dictation_request_id: None,
            #[cfg(feature = "dictation")]
            next_dictation_request_id: 1,
            #[cfg(feature = "dictation")]
            last_dictation_audio: None,
            runtime: RuntimeSettingsState::new(),
            approval: ApprovalState::new(),
            viewport: ViewportState::new(),
            context_usage: None,
            perf: None,
        }
    }

    // --- Configuration & status ---

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

    pub(super) fn reset_for_forked_thread(&mut self, thread_id: impl Into<String>) {
        self.thread_id = thread_id.into();
        self.active_turn_id = None;
        self.messages.clear();
        self.render_cache = RenderCacheState::new();
        self.agent_item_to_index.clear();
        self.turn_diff_to_index.clear();
        self.command_render_overrides.clear();
        self.input_history = InputHistoryState::new();
        self.turn_start_message_idx = None;
        self.viewport = ViewportState::new();
        self.context_usage = None;
        self.clear_pending_approval();
    }

    pub(super) fn set_pending_approval(&mut self, approval: PendingApprovalRequest) {
        self.status = format!("approval requested: {}", approval.title);
        self.approval.pending = Some(approval);
    }

    pub(super) fn clear_pending_approval(&mut self) {
        self.approval.pending = None;
    }

    // --- Turn input queue ---

    pub(super) fn queue_ralph_continuation(&mut self, text: impl Into<String>) {
        self.ralph_runtime.queue_continuation(text);
    }

    #[cfg(test)]
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

    pub(super) fn has_ready_queued_turn_input(&self, now: Instant) -> bool {
        self.ralph_runtime.has_ready_queued_turn_input(now)
    }

    pub(super) fn promote_ready_continuation(&mut self, now: Instant) {
        self.ralph_runtime.promote_ready_continuation(now);
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

    // --- Turn lifecycle ---

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
        let outcome =
            self.ralph_runtime
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

    // --- Query helpers & test utilities ---

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
