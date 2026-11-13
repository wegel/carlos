//! Admit automatic and queued prompts only after session guards permit them.

use std::time::Instant;

use crate::backend::{BackendKind, PromptOrigin};

use super::claude::ClaudePhase;
use super::codex::{goals, queue};
use super::ralph_runtime_state::QueuedTurnInput;
use super::state::SessionState;

impl SessionState {
    /// Consume one due prompt after the client confirms that its event buffers are clear.
    pub(crate) fn take_dispatchable_prompt(
        &mut self,
        kind: BackendKind,
        client_idle: bool,
        now: Instant,
    ) -> Option<QueuedTurnInput> {
        if kind == BackendKind::Claude && self.claude().phase() != ClaudePhase::Ready {
            return None;
        }
        if !client_idle || !queue::allows_automatic(self, self.codex_scope()) {
            return None;
        }
        if self.ralph_runtime().next_input_origin(now) == Some(PromptOrigin::Automatic)
            && goals::ralph_error(self, self.codex_scope()).is_some()
        {
            return None;
        }
        self.dequeue_turn_input(now)
    }
}
