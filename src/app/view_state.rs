//! Own terminal widgets, cached rows, and local display choices.

use std::cell::Cell;
use std::time::Instant;

use carlos_core::session::entry_id::EntryId;
use carlos_proto::ids::ClientId;
use carlos_proto::submissions::ClientOperation;
use ratatui_textarea::TextArea;
use uuid::Uuid;

use super::claude_edit_view::ClaudeEditView;
use super::codex_state::CodexCallbacksView;
#[cfg(feature = "dictation")]
use super::dictation_state::DictationCaptureState;
use super::dictation_state::DictationRuntimeState;
use super::input::make_input_area;
use super::input_history_state::InputHistoryView;
use super::layout_state::LayoutState;
use super::perf::PerfMetrics;
use super::render_cache_state::RenderCacheState;
use super::runtime_settings_state::RuntimeSettingsView;
use super::state_approval_view::ApprovalHints;
use super::tool_output_state::ToolOutputState;
use super::viewport_state::ViewportState;

pub(super) struct ViewState {
    pub(super) client_id: ClientId,
    next_operation: Cell<u64>,
    pub(super) core_cursor: carlos_proto::ids::Sequence,
    pub(super) codex_panel: super::codex_panel_state::PanelView,
    pub(super) codex_settings: super::codex_settings_view::SettingsView,
    pub(super) codex_goals: super::codex_goal_state::GoalView,
    pub(super) codex_queue: super::codex_queue_state::QueueView,
    pub(super) claude_answers: super::claude_question_state::NativeAnswers,
    pub(super) codex_callbacks: CodexCallbacksView,
    pub(super) claude_tasks: super::claude_tasks::ClaudeTasksView,
    pub(super) claude_queue: bool,
    pub(super) claude_unknown_selection: Option<usize>,
    pub(super) claude_edit: ClaudeEditView,
    pub(super) claude_quota_next_draw: Option<Instant>,
    pub(super) claude_settings_edit: Option<(String, String)>,
    pub(super) quota_next_draw: Option<Instant>,
    pub(super) summary_enabled_draft: bool,
    pub(super) input_history: InputHistoryView,
    pub(super) runtime_settings: RuntimeSettingsView,
    pub(super) render_cache: RenderCacheState,
    pub(super) tool_output: ToolOutputState,
    pub(super) layout: LayoutState,
    pub(super) input: TextArea<'static>,
    pub(super) esc_armed_at: Option<Instant>,
    pub(super) esc_interrupted_at: Option<Instant>,
    pub(super) dictation: DictationRuntimeState,
    #[cfg(feature = "dictation")]
    pub(super) dictation_io: DictationCaptureState,
    pub(super) viewport: ViewportState,
    pub(super) perf: Option<PerfMetrics>,
    pub(super) status_hint: Option<String>,
    pub(super) approval_hints: ApprovalHints,
    pub(super) confirm_allow: Option<EntryId>,
}

impl ViewState {
    pub(super) fn new(summary_enabled_draft: bool) -> Self {
        Self {
            client_id: Uuid::new_v4()
                .simple()
                .to_string()
                .parse()
                .expect("client id"),
            next_operation: Cell::new(0),
            core_cursor: Default::default(),
            codex_panel: Default::default(),
            codex_settings: Default::default(),
            codex_goals: Default::default(),
            codex_queue: Default::default(),
            claude_answers: Default::default(),
            codex_callbacks: CodexCallbacksView::default(),
            claude_tasks: Default::default(),
            claude_queue: false,
            claude_unknown_selection: None,
            claude_edit: ClaudeEditView::default(),
            claude_quota_next_draw: None,
            claude_settings_edit: None,
            quota_next_draw: None,
            summary_enabled_draft,
            input_history: InputHistoryView::default(),
            runtime_settings: RuntimeSettingsView::new(),
            render_cache: RenderCacheState::new(),
            tool_output: ToolOutputState::default(),
            layout: LayoutState::default(),
            input: make_input_area(),
            esc_armed_at: None,
            esc_interrupted_at: None,
            dictation: DictationRuntimeState::disabled("dictation feature is not configured"),
            #[cfg(feature = "dictation")]
            dictation_io: DictationCaptureState::default(),
            viewport: ViewportState::new(),
            perf: None,
            status_hint: None,
            approval_hints: ApprovalHints::default(),
            confirm_allow: None,
        }
    }
}

impl ViewState {
    pub(super) fn new_operation(&self) -> ClientOperation {
        let next = self
            .next_operation
            .get()
            .checked_add(1)
            .expect("operation counter exhausted");
        self.next_operation.set(next);
        ClientOperation {
            client: self.client_id.clone(),
            id: carlos_proto::ids::OperationId::new(next).expect("nonzero operation"),
        }
    }
    pub(super) fn owns_operation(&self, operation: Option<&ClientOperation>) -> bool {
        operation.is_none_or(|operation| operation.client == self.client_id)
    }
}
