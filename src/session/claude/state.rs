//! Keep native phase, model inheritance, and read facts in one session.

use serde_json::Value;

use super::context::ClaudeContext;
use super::quota::ClaudeQuota;
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

    /// Record the phase at the existing native lifecycle boundary.
    pub(crate) fn set_claude_phase(&mut self, phase: ClaudePhase) {
        self.claude.phase = phase;
    }

    /// Retire helper metadata and optional reads while retaining explicit-choice facts.
    pub(crate) fn reset_claude_metadata(&mut self) {
        self.claude = ClaudeState {
            inherits_model: self.claude.inherits_model,
            inherits_effort: self.claude.inherits_effort,
            ..ClaudeState::default()
        };
    }
}
