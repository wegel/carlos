//! Application state: transcript, input, display mode, and sub-state aggregation.

// --- Imports ---

use std::path::PathBuf;
#[cfg(test)]
use std::time::Instant;

#[cfg(test)]
use carlos_core::session::approval_state::ApprovalRequestDraft;
#[cfg(test)]
use carlos_core::session::ralph::RalphConfig;
#[cfg(test)]
use carlos_core::session::ralph_runtime_state::QueuedTurnInput;
use carlos_core::session::state::SessionState;

use super::perf::PerfMetrics;
use super::view_state::ViewState;

pub(super) use super::runtime_settings_state::ModelSettingsField;

// --- Types ---

pub(super) struct AppState {
    pub(super) session: SessionState,
    pub(super) view: ViewState,
}

// --- Construction ---

impl AppState {
    #[cfg(test)]
    pub(super) fn new(thread_id: String) -> Self {
        Self::with_runtime_defaults(
            thread_id,
            carlos_core::session::runtime_settings::RuntimeDefaults::default(),
        )
    }

    /// Pass startup choices to the session without letting it read client files.
    pub(super) fn with_runtime_defaults(
        thread_id: String,
        preferences: carlos_core::session::runtime_settings::RuntimeDefaults,
    ) -> Self {
        let summary_enabled = super::tool_output_preferences::load_summary_enabled();
        Self {
            session: SessionState::new(thread_id, preferences, summary_enabled),
            view: ViewState::new(summary_enabled),
        }
    }

    // --- Configuration & status ---

    pub(super) fn enable_perf_metrics(&mut self) {
        self.view.perf = Some(PerfMetrics::new());
    }

    pub(super) fn configure_ralph_options(
        &mut self,
        cwd: PathBuf,
        prompt_path_override: Option<String>,
        done_marker_override: Option<String>,
        blocked_marker_override: Option<String>,
        waiting_marker_override: Option<String>,
    ) {
        self.session.configure_ralph_options(
            cwd,
            prompt_path_override,
            done_marker_override,
            blocked_marker_override,
            waiting_marker_override,
        )
    }

    #[cfg(test)]
    pub(super) fn enable_ralph_mode(&mut self, config: RalphConfig) {
        self.session.enable_ralph_mode(config)
    }

    pub(super) fn disable_ralph_mode(&mut self) {
        self.session.disable_ralph_mode()
    }

    pub(super) fn perf_report(&self) -> Option<String> {
        self.view.perf.as_ref().map(PerfMetrics::final_report)
    }

    /// Store a backend, turn, or Ralph fact; local notices use `set_hint`.
    pub(super) fn set_status(&mut self, s: impl Into<String>) {
        self.session.set_status(s)
    }

    #[cfg(test)]
    pub(super) fn set_pending_approval(&mut self, approval: ApprovalRequestDraft) {
        let before = self.session.approval().pending().map(|p| p.entry_id());
        let retired = self.session.set_pending_approval(approval);
        let current = self.session.approval().pending().map(|p| p.entry_id());
        // A duplicate delivery keeps the active card; only its extra hint lines reset.
        if current == before {
            self.view.approval_hints.clear_extra_lines(current);
        }
        self.drop_retired_approval_hints(retired);
    }

    #[cfg(test)]
    pub(super) fn clear_pending_approval(&mut self) {
        let cleared = self.session.clear_pending_session_approval();
        self.apply_cleared_approval(cleared);
    }

    /// Discard only the editor and hints named by the session's retirement result.
    pub(super) fn apply_cleared_approval(
        &mut self,
        cleared: carlos_core::session::approval_reply::ApprovalCleared,
    ) {
        match cleared {
            carlos_core::session::approval_reply::ApprovalCleared::Codex(retired) => {
                super::codex_callbacks::apply_retired(self, retired);
            }
            carlos_core::session::approval_reply::ApprovalCleared::Other { editor, cards } => {
                if let Some(id) = editor {
                    self.view.claude_answers.drafts.remove(&id);
                }
                self.drop_retired_approval_hints(cards);
            }
        }
    }

    // --- Turn input queue ---

    #[cfg(test)]
    pub(super) fn queue_ralph_continuation(&mut self, text: impl Into<String>) {
        self.session.queue_ralph_continuation(text)
    }

    #[cfg(test)]
    pub(super) fn queue_turn_input(&mut self, text: impl Into<String>) {
        self.session.queue_turn_input(text)
    }

    #[cfg(test)]
    pub(super) fn queue_turn_input_with_history(
        &mut self,
        text: impl Into<String>,
        record_input_history: bool,
    ) {
        self.session
            .queue_turn_input_with_history(text, record_input_history)
    }

    pub(super) fn has_pending_ralph_continuation(&self) -> bool {
        self.session.has_pending_ralph_continuation()
    }

    #[cfg(test)]
    pub(super) fn dequeue_turn_input(&mut self, now: Instant) -> Option<QueuedTurnInput> {
        self.session.dequeue_turn_input(now)
    }

    // --- Turn lifecycle ---

    #[cfg(test)]
    pub(super) fn record_turn_started(&mut self, turn_id: &str) -> bool {
        self.session.record_turn_started(turn_id)
    }

    // --- Query helpers & test utilities ---

    pub(super) fn ralph_enabled(&self) -> bool {
        self.session.ralph_enabled()
    }

    pub(super) fn rewind_mode(&self) -> bool {
        self.view.input_history.rewind_mode()
    }
}
