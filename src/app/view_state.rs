//! Own only this terminal's widgets, private composer, render cache and display choices.

use std::time::{Duration, Instant};

use carlos_core::session::entry_id::EntryId;
use ratatui_textarea::TextArea;

#[cfg(feature = "dictation")]
use super::dictation_state::DictationCaptureState;
use super::dictation_state::DictationRuntimeState;
use super::input::make_input_area;
use super::layout_state::LayoutState;
use super::perf::PerfMetrics;
use super::render_cache_state::RenderCacheState;
use super::runtime_settings_state::RuntimeSettingsView;
use super::tool_output_state::ToolOutputState;
use super::viewport_state::ViewportState;

const ESC_CHORD_WINDOW: Duration = Duration::from_millis(700);

pub(super) struct ViewState {
    pub(super) codex_goals: super::codex_goal_state::GoalView,
    pub(super) codex_queue: super::codex_queue_state::QueueView,
    pub(super) summary_enabled_draft: bool,
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
    pub(super) confirm_allow: Option<EntryId>,
}

impl ViewState {
    pub(super) fn new(summary_enabled_draft: bool) -> Self {
        Self {
            codex_goals: Default::default(),
            codex_queue: Default::default(),
            summary_enabled_draft,
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
            confirm_allow: None,
        }
    }
}

impl ViewState {
    pub(super) fn reset_esc_chord(&mut self) {
        self.esc_armed_at = None;
        self.esc_interrupted_at = None;
    }

    /// Require a quiet Escape interval after interrupting native work.
    pub(super) fn suppress_esc_chord_after_interrupt(&mut self, now: Instant) {
        self.esc_armed_at = None;
        self.esc_interrupted_at = Some(now);
    }

    #[cfg(test)]
    pub(super) fn expire_esc_chord(&mut self, now: Instant) {
        if self
            .esc_armed_at
            .is_some_and(|at| now.duration_since(at) > ESC_CHORD_WINDOW)
        {
            self.esc_armed_at = None;
        }
    }

    /// Complete only a second quiet Escape within the existing 700 ms interval.
    pub(super) fn register_escape_press(&mut self, now: Instant) -> bool {
        if self
            .esc_interrupted_at
            .take()
            .is_some_and(|at| now.duration_since(at) <= ESC_CHORD_WINDOW)
        {
            self.suppress_esc_chord_after_interrupt(now);
            return false;
        }
        if self
            .esc_armed_at
            .is_some_and(|at| now.duration_since(at) <= ESC_CHORD_WINDOW)
        {
            self.esc_armed_at = None;
            return true;
        }
        self.esc_armed_at = Some(now);
        false
    }
}
