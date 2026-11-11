//! Keep queued human input ahead of delayed Carlos automatic continuations.

use std::time::{Duration, Instant};

use super::{QueuedTurnInput, RalphRuntimeState, RALPH_CONTINUATION_DELAY};
use crate::backend::PromptOrigin;

impl RalphRuntimeState {
    /// Queue actual human text without changing its source.
    pub(in crate::app) fn enqueue_turn_input(
        &mut self,
        text: impl Into<String>,
        record_input_history: bool,
    ) {
        self.enqueue_prompt(text.into(), record_input_history, PromptOrigin::Human);
    }

    /// Keep prompt text and its actual human/automatic source together.
    pub(in crate::app) fn enqueue_prompt(
        &mut self,
        text: String,
        record_input_history: bool,
        origin: PromptOrigin,
    ) {
        if text.trim().is_empty() {
            return;
        }
        self.queued_turn_inputs.push_back(QueuedTurnInput {
            text,
            record_input_history,
            origin,
            keeps_ralph_pending: false,
            available_at: None,
        });
    }

    /// Delay one Carlos automatic continuation so human input can arrive first.
    pub(in crate::app) fn queue_continuation(&mut self, text: impl Into<String>) {
        let text = text.into();
        if text.trim().is_empty() {
            return;
        }
        self.pending_continuation = Some(QueuedTurnInput {
            text,
            record_input_history: false,
            origin: PromptOrigin::Automatic,
            keeps_ralph_pending: true,
            available_at: Some(Instant::now() + RALPH_CONTINUATION_DELAY),
        });
    }

    /// Report whether a delayed or queued Carlos continuation still exists.
    pub(in crate::app) fn has_pending_continuation(&self) -> bool {
        self.pending_continuation.is_some()
            || self
                .queued_turn_inputs
                .iter()
                .any(|turn| turn.keeps_ralph_pending)
    }

    /// Wake only when a delayed continuation can become ready.
    pub(in crate::app) fn pending_continuation_wait(&self, now: Instant) -> Option<Duration> {
        if !self.queued_turn_inputs.is_empty() {
            return None;
        }
        self.pending_continuation
            .as_ref()
            .and_then(|turn| turn.available_at)
            .map(|deadline| deadline.saturating_duration_since(now))
            .filter(|wait| !wait.is_zero())
    }

    #[cfg(test)]
    /// Expose the fixed continuation deadline to deterministic tests.
    pub(in crate::app) fn pending_continuation_deadline(&self) -> Option<Instant> {
        self.pending_continuation
            .as_ref()
            .and_then(|turn| turn.available_at)
    }

    /// Move one due continuation behind already queued inputs.
    pub(in crate::app) fn promote_ready_continuation(&mut self, now: Instant) {
        let ready = self
            .pending_continuation
            .as_ref()
            .and_then(|turn| turn.available_at)
            .is_some_and(|deadline| now >= deadline);
        if !ready {
            return;
        }
        if let Some(mut continuation) = self.pending_continuation.take() {
            continuation.available_at = None;
            self.queued_turn_inputs.push_back(continuation);
        }
    }

    /// Consume the next ready input once without resubmitting it.
    pub(in crate::app) fn dequeue_turn_input(&mut self, now: Instant) -> Option<QueuedTurnInput> {
        self.promote_ready_continuation(now);
        if let Some(turn) = self.queued_turn_inputs.pop_front() {
            return Some(turn);
        }
        None
    }

    #[cfg(test)]
    /// Check queued work in deterministic runtime tests.
    pub(in crate::app) fn queued_turn_inputs_is_empty(&self) -> bool {
        self.queued_turn_inputs.is_empty()
    }

    /// Report work that can run now without shortening its human gate.
    pub(in crate::app) fn has_ready_queued_turn_input(&self, now: Instant) -> bool {
        if !self.queued_turn_inputs.is_empty() {
            return true;
        }
        self.pending_continuation
            .as_ref()
            .and_then(|turn| turn.available_at)
            .is_some_and(|deadline| now >= deadline)
    }

    /// Peek without consuming work so native goals can hold automatic prompts while humans remain usable.
    pub(in crate::app) fn next_input_origin(&self, now: Instant) -> Option<PromptOrigin> {
        self.queued_turn_inputs
            .front()
            .map(|turn| turn.origin)
            .or_else(|| {
                self.pending_continuation
                    .as_ref()
                    .filter(|turn| turn.available_at.is_some_and(|at| at <= now))
                    .map(|turn| turn.origin)
            })
    }
}
