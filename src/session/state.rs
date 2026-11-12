//! Own transcript and turn facts independently of a terminal client.

use std::collections::{HashMap, HashSet};

use super::approval_state::ApprovalState;
use super::context_usage::ContextUsage;
use super::entry_key::BackendEntryKey;
use super::input_history::PromptHistory;
use super::models::TurnStatus;
use super::ralph_runtime_state::RalphRuntimeState;
use super::rate_limits::RateLimits;
use super::runtime_settings::{RuntimeDefaults, RuntimeSettingsState};
use super::summary_state::SummaryState;
use super::transcript::Transcript;
use super::turns::TurnBoundary;

/// Own the facts that remain true without a terminal view.
#[cfg_attr(test, derive(Debug))]
pub(crate) struct SessionState {
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
    pub(crate) fn new(
        thread_id: String,
        preferences: RuntimeDefaults,
        summary_enabled: bool,
    ) -> Self {
        Self {
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

    /// Borrow the current thread id.
    pub(crate) fn thread_id(&self) -> &String {
        &self.thread_id
    }

    /// Borrow the current entry conversation.
    pub(crate) fn entry_conversation(&self) -> &String {
        &self.entry_conversation
    }

    /// Borrow the current thread name.
    pub(crate) fn thread_name(&self) -> &Option<String> {
        &self.thread_name
    }

    /// Borrow the current active turn id.
    pub(crate) fn active_turn_id(&self) -> &Option<String> {
        &self.active_turn_id
    }

    /// Borrow the current messages.
    pub(crate) fn messages(&self) -> &Transcript {
        &self.messages
    }

    /// Borrow the current completed item ids.
    #[cfg(test)]
    pub(crate) fn completed_item_ids(&self) -> &HashSet<BackendEntryKey> {
        &self.completed_item_ids
    }

    /// Borrow the current turn status by id.
    #[cfg(test)]
    pub(crate) fn turn_status_by_id(&self) -> &HashMap<String, TurnStatus> {
        &self.turn_status_by_id
    }

    /// Borrow the current status.
    #[cfg(test)]
    pub(crate) fn status(&self) -> &String {
        &self.status
    }

    /// Borrow the current ralph runtime.
    pub(crate) fn ralph_runtime(&self) -> &RalphRuntimeState {
        &self.ralph_runtime
    }

    /// Borrow the current approval.
    pub(crate) fn approval(&self) -> &ApprovalState {
        &self.approval
    }

    /// Borrow the current context usage.
    pub(crate) fn context_usage(&self) -> Option<ContextUsage> {
        self.context_usage
    }
}
