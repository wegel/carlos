//! Keep native helper, process, model, and read facts in one session.

use std::collections::{HashMap, VecDeque};

use serde_json::Value;

use super::activity::ClaudeActivity;
use super::browser::BrowserRequest;
use super::context::ClaudeContext;
use super::edit::ClaudeEdit;
use super::history::ClaudeHistory;
use super::prompts::ClaudePrompts;
use super::quota::ClaudeQuota;
use super::tasks::ClaudeTasks;
use crate::session::state::SessionState;

/// Distinguish a starting query from a ready or permanently closed helper.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClaudePhase {
    Starting,
    Ready,
    Closed,
}

/// Own native Claude facts independently of local settings and countdowns.
#[cfg_attr(test, derive(Debug))]
pub(crate) struct ClaudeState {
    pub(in crate::session) enabled: bool,
    pub(in crate::session) generation: u64,
    pub(in crate::session) epoch: u64,
    pub(in crate::session) notices: VecDeque<String>,
    pub(in crate::session) reset_key: Option<String>,
    pub(in crate::session) awaiting_reset_init: bool,
    pub(in crate::session) history: ClaudeHistory,
    pub(in crate::session) prompts: ClaudePrompts,
    pub(in crate::session) edit: ClaudeEdit,
    pub(in crate::session) tasks: ClaudeTasks,
    pub(in crate::session) activity: ClaudeActivity,
    pub(in crate::session) browser: HashMap<(String, String), BrowserRequest>,
    pub(in crate::session) phase: ClaudePhase,
    pub(in crate::session) quota: ClaudeQuota,
    pub(in crate::session) context: ClaudeContext,
    pub(in crate::session) capabilities: Option<Value>,
    pub(in crate::session) observed_model: Option<String>,
    pub(in crate::session) inherits_model: bool,
    pub(in crate::session) inherits_effort: bool,
}

impl Default for ClaudeState {
    fn default() -> Self {
        Self {
            enabled: false,
            generation: 1,
            epoch: 0,
            notices: VecDeque::new(),
            reset_key: None,
            awaiting_reset_init: false,
            history: ClaudeHistory::default(),
            prompts: ClaudePrompts::default(),
            edit: ClaudeEdit::default(),
            tasks: ClaudeTasks::default(),
            activity: ClaudeActivity::default(),
            browser: HashMap::new(),
            phase: ClaudePhase::Ready,
            quota: ClaudeQuota::default(),
            context: ClaudeContext::default(),
            capabilities: None,
            observed_model: None,
            inherits_model: true,
            inherits_effort: true,
        }
    }
}

impl ClaudeState {
    /// Read whether this app has received or started Claude work.
    pub(crate) fn enabled(&self) -> bool {
        self.enabled
    }

    /// Read the current replacement helper's generation.
    #[cfg(test)]
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    /// Borrow native task records and stop progress.
    pub(crate) fn tasks(&self) -> &ClaudeTasks {
        &self.tasks
    }

    /// Borrow main-process activity and context fences.
    pub(crate) fn activity(&self) -> &ClaudeActivity {
        &self.activity
    }

    /// Count native browser requests without exposing mutable callback facts.
    #[cfg(test)]
    pub(crate) fn browser_request_count(&self) -> usize {
        self.browser.len()
    }

    /// Read whether the native helper can accept prompts.
    pub(crate) fn phase(&self) -> ClaudePhase {
        self.phase
    }

    /// Borrow the native model metadata.
    #[cfg(test)]
    pub(crate) fn capabilities(&self) -> Option<&Value> {
        self.capabilities.as_ref()
    }

    /// Borrow the model most recently reported by native Claude.
    #[cfg(test)]
    pub(crate) fn observed_model(&self) -> Option<&str> {
        self.observed_model.as_deref()
    }

    /// Read whether native settings choose the model.
    #[cfg(test)]
    pub(crate) fn inherits_model(&self) -> bool {
        self.inherits_model
    }

    /// Borrow quota windows and their next backend read deadline.
    pub(crate) fn quota(&self) -> &ClaudeQuota {
        &self.quota
    }

    /// Read whether the last root-context count is still fresh.
    pub(crate) fn context_fresh(&self) -> bool {
        self.context.fresh
    }
}

impl SessionState {
    /// Borrow Claude facts without granting mutable access.
    pub(crate) fn claude(&self) -> &ClaudeState {
        &self.claude
    }

    /// Enable Claude when startup or a named fact identifies this backend.
    pub(crate) fn enable_claude(&mut self) {
        self.claude.enabled = true;
    }
}
