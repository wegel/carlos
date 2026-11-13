//! Track turn starts, terminal states, and Ralph's exact transcript suffix.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::models::{Role, TurnStatus};
use super::ralph_runtime_state::QueuedTurnInput;
use super::state::SessionState;
#[cfg(any(test, feature = "test-support"))]
use crate::session::ralph::RalphConfig;

/// Keep the suffix boundary even when no entry existed at turn start.
#[cfg_attr(any(test, feature = "test-support"), derive(Debug))]
pub(super) struct TurnBoundary {
    pub(super) offset: usize,
}

impl SessionState {
    /// Forget a completed or failed turn's suffix boundary.
    pub(crate) fn clear_turn_boundary(&mut self) {
        self.turn_boundary = None;
    }

    /// Resolve the saved suffix without scanning or copying entries.
    pub(super) fn turn_suffix_start(&self) -> Option<usize> {
        self.turn_boundary.as_ref().map(|boundary| boundary.offset)
    }

    #[cfg(any(test, feature = "test-support"))]
    /// Inspect the private boundary in parity tests without exposing a saved app cursor.
    pub fn turn_start(&self) -> Option<usize> {
        self.turn_suffix_start()
    }

    /// Resolve the active reasoning row for an in-process renderer.
    pub fn active_reasoning_step(&self) -> Option<usize> {
        self.active_turn_id.as_ref()?;
        let start = self.turn_suffix_start().unwrap_or(0);
        let index = self.messages.active_step(start)?;
        (self.messages[index].role == Role::Reasoning).then_some(index)
    }

    /// Store the supplied prompt path and stop markers for later toggles.
    pub fn configure_ralph_options(
        &mut self,
        cwd: PathBuf,
        prompt_path_override: Option<String>,
        done_marker_override: Option<String>,
        blocked_marker_override: Option<String>,
        waiting_marker_override: Option<String>,
    ) {
        self.ralph_runtime.configure_options(
            cwd,
            prompt_path_override,
            done_marker_override,
            blocked_marker_override,
            waiting_marker_override,
        );
    }

    #[cfg(any(test, feature = "test-support"))]
    /// Enable Ralph with the supplied plain configuration and report its prompt path.
    pub fn enable_ralph_mode(&mut self, config: RalphConfig) {
        let status = self.ralph_runtime.enable(config);
        self.set_status(status);
    }

    /// Clear Ralph and its queued continuations before reporting that it is off.
    pub fn disable_ralph_mode(&mut self) {
        let status = self.ralph_runtime.disable();
        self.set_status(status);
    }

    /// Delay a continuation so newly queued human text can go first.
    pub fn queue_ralph_continuation(&mut self, text: impl Into<String>) {
        self.ralph_runtime.queue_continuation(text);
    }

    #[cfg(any(test, feature = "test-support"))]
    /// Queue human text with input-history recording enabled.
    pub fn queue_turn_input(&mut self, text: impl Into<String>) {
        self.queue_turn_input_with_history(text, true);
    }

    /// Queue human text with the supplied history-recording choice.
    pub fn queue_turn_input_with_history(
        &mut self,
        text: impl Into<String>,
        record_input_history: bool,
    ) {
        self.ralph_runtime
            .enqueue_turn_input(text, record_input_history);
    }

    /// Check whether queued text or a due continuation can run now.
    pub(crate) fn has_ready_queued_turn_input(&self, now: Instant) -> bool {
        self.ralph_runtime.has_ready_queued_turn_input(now)
    }

    /// Place a due continuation after already queued prompts.
    pub fn promote_ready_continuation(&mut self, now: Instant) {
        self.ralph_runtime.promote_ready_continuation(now);
    }

    /// Check both the delayed continuation and the ready queue.
    pub fn has_pending_ralph_continuation(&self) -> bool {
        self.ralph_runtime.has_pending_continuation()
    }

    /// Return the remaining delay only when the ready queue is empty.
    pub(crate) fn pending_ralph_continuation_wait(&self, now: Instant) -> Option<Duration> {
        self.ralph_runtime.pending_continuation_wait(now)
    }

    /// Promote due continuations and consume one ready prompt.
    pub fn dequeue_turn_input(&mut self, now: Instant) -> Option<QueuedTurnInput> {
        self.ralph_runtime.dequeue_turn_input(now)
    }

    /// Append the context notice unless the last row already contains it.
    pub(crate) fn append_context_compacted_marker(&mut self) {
        const MARKER: &str = "↻ Context compacted";
        if let Some(last) = self.messages.last() {
            if last.role == Role::System && last.text == MARKER {
                return;
            }
        }
        self.append_message(Role::System, MARKER);
    }

    /// Append the interruption notice unless the last row already contains it.
    pub fn append_turn_interrupted_marker(&mut self) {
        const MARKER: &str = "Turn interrupted";
        if let Some(last) = self.messages.last() {
            if last.role == Role::System && last.text == MARKER {
                return;
            }
        }
        self.append_message(Role::System, MARKER);
    }

    /// Save an empty suffix boundary at the current transcript end.
    pub(crate) fn mark_turn_started(&mut self) {
        self.events.turn_started_at = Some(super::usage::unix_now());
        self.turn_boundary = Some(TurnBoundary {
            offset: self.messages.len(),
        });
    }

    /// Ignore duplicate or terminal starts before recording a new active turn.
    pub fn record_turn_started(&mut self, turn_id: &str) -> bool {
        if self.active_turn_id.as_deref() == Some(turn_id)
            || self
                .turn_status_by_id
                .get(turn_id)
                .is_some_and(|status| status.is_terminal())
        {
            return false;
        }
        self.active_turn_id = Some(turn_id.to_string());
        self.turn_status_by_id
            .insert(turn_id.to_string(), TurnStatus::InProgress);
        self.mark_turn_started();
        true
    }

    /// Record terminal status and clear only the matching active turn.
    pub(crate) fn record_turn_completed(&mut self, turn_id: &str, status: TurnStatus) -> bool {
        if !status.is_terminal() {
            return false;
        }
        self.turn_status_by_id.insert(turn_id.to_string(), status);
        if self.active_turn_id.as_deref() != Some(turn_id) {
            return false;
        }
        self.active_turn_id = None;
        true
    }

    /// Store a replayed turn status without changing the active turn.
    pub(crate) fn record_historical_turn_status(&mut self, turn_id: &str, status: TurnStatus) {
        self.turn_status_by_id.insert(turn_id.to_string(), status);
    }

    /// Scan the saved suffix and append a notice before disabling blocked Ralph work.
    pub fn maybe_disable_ralph_on_blocked_marker(&mut self) {
        let start_idx = self.turn_suffix_start().unwrap_or(self.messages.len());
        let Some(outcome) = self
            .ralph_runtime
            .blocked_marker_outcome(&self.messages, start_idx)
        else {
            return;
        };

        if let Some(msg) = outcome.system_message {
            if !self
                .messages
                .last()
                .is_some_and(|last| last.role == Role::System && last.text == msg)
            {
                self.append_message(Role::System, msg);
            }
        }
        if outcome.disable {
            self.disable_ralph_mode();
        }
        if let Some(status) = outcome.status {
            self.set_status(status);
        }
    }

    /// Consume the saved suffix before applying Ralph notices, continuations, and status.
    pub fn handle_ralph_turn_completed(&mut self, interrupted: bool) {
        let start_idx = self
            .turn_boundary
            .take()
            .map(|boundary| boundary.offset)
            .unwrap_or(self.messages.len());
        let outcome =
            self.ralph_runtime
                .handle_turn_completed(&self.messages, start_idx, interrupted);

        if let Some(msg) = outcome.system_message {
            if !self
                .messages
                .last()
                .is_some_and(|last| last.role == Role::System && last.text == msg)
            {
                self.append_message(Role::System, msg);
            }
        }
        if let Some(text) = outcome.continuation {
            self.queue_ralph_continuation(text);
        }
        if outcome.disable {
            self.disable_ralph_mode();
        }
        if let Some(status) = outcome.status {
            self.set_status(status);
        }
    }

    /// Check whether Ralph currently owns an automatic loop.
    pub fn ralph_enabled(&self) -> bool {
        self.ralph_runtime.is_enabled()
    }

    #[cfg(any(test, feature = "test-support"))]
    /// Check the ready queue in deterministic tests.
    pub fn queued_turn_inputs_is_empty(&self) -> bool {
        self.ralph_runtime.queued_turn_inputs_is_empty()
    }

    #[cfg(any(test, feature = "test-support"))]
    /// Check whether a user deferred a toggle until the turn ends.
    pub fn ralph_toggle_pending(&self) -> bool {
        self.ralph_runtime.toggle_pending()
    }

    #[cfg(any(test, feature = "test-support"))]
    /// Expose the fixed delay to deterministic tests.
    pub fn ralph_pending_continuation_deadline(&self) -> Option<Instant> {
        self.ralph_runtime.pending_continuation_deadline()
    }

    #[cfg(any(test, feature = "test-support"))]
    /// Check the current Ralph waiting flag in tests.
    pub fn ralph_waiting_for_user(&self) -> bool {
        self.ralph_runtime
            .state()
            .is_some_and(|ralph| ralph.waiting_for_user)
    }
}
