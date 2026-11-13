//! Own transcript and turn facts independently of a terminal client.

use std::collections::{HashMap, HashSet};

use super::approval_state::ApprovalState;
use super::claude::ClaudeState;
use super::codex::callbacks::CodexCallbacks;
use super::context_usage::ContextUsage;
use super::entry_key::BackendEntryKey;
use super::input_history::PromptHistory;
use super::ralph_runtime_state::RalphRuntimeState;
use super::rate_limits::RateLimits;
use super::runtime_settings::{RuntimeDefaults, RuntimeSettingsState};
use super::summary_state::SummaryState;
use super::transcript::Transcript;
use super::turns::TurnBoundary;
use crate::session::models::TurnStatus;

/// Own the facts that remain true without a terminal view.
///
/// A client submits commands instead of writing the session's stored facts:
///
/// ```compile_fail
/// use carlos_core::session::state::SessionState;
/// fn overwrite(session: &mut SessionState) {
///     session.thread_id = String::from("another-thread");
/// }
/// ```
#[cfg_attr(any(test, feature = "test-support"), derive(Debug))]
pub struct SessionState {
    pub(in crate::session) commands: super::command_guard::CommandState,
    pub(in crate::session) events: super::events::EventState,
    pub(in crate::session) codex_connection: super::codex::connection_state::ConnectionState,
    pub(in crate::session) codex_panel: super::codex::panel_state::PanelState,
    pub(in crate::session) codex_task_status: super::codex::task_status::TaskStatus,
    pub(in crate::session) codex_settings: super::codex::settings_state::SettingsState,
    pub(in crate::session) codex_goals: super::codex::goal_state::GoalState,
    pub(in crate::session) codex_queue: super::codex::queue_state::QueueState,
    pub(in crate::session) codex_meta: super::codex::metadata::CodexMetadata,
    pub(in crate::session) claude: ClaudeState,
    pub(in crate::session) codex_callbacks: CodexCallbacks,
    pub(in crate::session) rate_limits: RateLimits,
    pub(in crate::session) summaries: SummaryState,
    pub(in crate::session) input_history: PromptHistory,
    pub(in crate::session) runtime: RuntimeSettingsState,
    pub(in crate::session) thread_id: String,
    pub(in crate::session) entry_conversation: String,
    pub(in crate::session) thread_name: Option<String>,
    pub(in crate::session) active_turn_id: Option<String>,
    pub(in crate::session) messages: Transcript,
    pub(in crate::session) completed_item_ids: HashSet<BackendEntryKey>,
    pub(in crate::session) turn_status_by_id: HashMap<String, TurnStatus>,
    pub(in crate::session) command_render_overrides: HashMap<BackendEntryKey, String>,
    pub(in crate::session) status: String,
    pub(in crate::session) ralph_runtime: RalphRuntimeState,
    pub(in crate::session) approval: ApprovalState,
    pub(in crate::session) context_usage: Option<ContextUsage>,
    pub(in crate::session) turn_boundary: Option<TurnBoundary>,
}

impl SessionState {
    /// Create an empty conversation without loading client preferences.
    pub fn new(thread_id: String, preferences: RuntimeDefaults, summary_enabled: bool) -> Self {
        Self {
            commands: Default::default(),
            events: super::events::EventState::new(),
            codex_connection: Default::default(),
            codex_panel: Default::default(),
            codex_task_status: Default::default(),
            codex_settings: Default::default(),
            codex_goals: Default::default(),
            codex_queue: Default::default(),
            codex_meta: Default::default(),
            claude: ClaudeState::default(),
            codex_callbacks: CodexCallbacks::default(),
            rate_limits: RateLimits::default(),
            summaries: SummaryState::new(summary_enabled),
            input_history: PromptHistory::default(),
            runtime: RuntimeSettingsState::new(preferences),
            entry_conversation: thread_id.clone(),
            thread_id,
            thread_name: None,
            active_turn_id: None,
            messages: Transcript::new(),
            completed_item_ids: HashSet::new(),
            turn_status_by_id: HashMap::new(),
            command_render_overrides: HashMap::new(),
            status: String::new(),
            ralph_runtime: RalphRuntimeState::new(),
            approval: ApprovalState::default(),
            context_usage: None,
            turn_boundary: None,
        }
    }

    /// Borrow unresolved Codex callback facts.
    pub fn codex_callbacks(&self) -> &CodexCallbacks {
        &self.codex_callbacks
    }

    /// Borrow the current thread id.
    pub fn thread_id(&self) -> &String {
        &self.thread_id
    }

    /// Borrow the current entry conversation.
    pub(crate) fn entry_conversation(&self) -> &String {
        &self.entry_conversation
    }

    /// Borrow the current thread name.
    pub fn thread_name(&self) -> &Option<String> {
        &self.thread_name
    }

    /// Borrow the current active turn id.
    pub fn active_turn_id(&self) -> &Option<String> {
        &self.active_turn_id
    }

    /// Borrow the current messages.
    pub fn messages(&self) -> &Transcript {
        &self.messages
    }

    /// Borrow the current completed item ids.
    #[cfg(any(test, feature = "test-support"))]
    pub fn completed_item_ids(&self) -> &HashSet<BackendEntryKey> {
        &self.completed_item_ids
    }

    /// Borrow the current turn status by id.
    #[cfg(any(test, feature = "test-support"))]
    pub fn turn_status_by_id(&self) -> &HashMap<String, TurnStatus> {
        &self.turn_status_by_id
    }

    /// Borrow the current status.
    #[cfg(any(test, feature = "test-support"))]
    pub fn status(&self) -> &String {
        &self.status
    }

    /// Borrow the current ralph runtime.
    pub fn ralph_runtime(&self) -> &RalphRuntimeState {
        &self.ralph_runtime
    }

    /// Borrow the current approval.
    pub fn approval(&self) -> &ApprovalState {
        &self.approval
    }

    /// Borrow the current context usage.
    pub fn context_usage(&self) -> Option<ContextUsage> {
        self.context_usage
    }
}
