//! Own terminal widgets, cached rows, and local display choices.

use std::time::Instant;

use ratatui_textarea::TextArea;

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
use crate::session::entry_id::EntryId;

pub(super) struct ViewState {
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
    pub(super) fn new() -> Self {
        Self {
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
