//! Application state: transcript, input, display mode, and sub-state aggregation.

// --- Imports ---

use std::path::PathBuf;

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

    pub(super) fn has_pending_ralph_continuation(&self) -> bool {
        self.session.has_pending_ralph_continuation()
    }

    // --- Turn lifecycle ---

    // --- Query helpers & test utilities ---

    pub(super) fn ralph_enabled(&self) -> bool {
        self.session.ralph_enabled()
    }

    pub(super) fn rewind_mode(&self) -> bool {
        self.view.input_history.rewind_mode()
    }
}
