use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;

use super::models::Message;
use super::ralph::{detect_turn_markers, load_ralph_config, RalphConfig, RalphState};

const RALPH_CONTINUATION_DELAY: Duration = Duration::from_millis(700);

pub(super) struct QueuedTurnInput {
    pub(super) text: String,
    pub(super) record_input_history: bool,
    keeps_ralph_pending: bool,
    available_at: Option<Instant>,
}

pub(super) struct RalphRuntimeState {
    current: Option<RalphState>,
    queued_turn_inputs: VecDeque<QueuedTurnInput>,
    toggle_pending: bool,
    pending_continuation: Option<QueuedTurnInput>,
    cwd: PathBuf,
    prompt_path_override: Option<String>,
    done_marker_override: Option<String>,
    blocked_marker_override: Option<String>,
}

pub(super) struct RalphTurnOutcome {
    pub(super) system_message: Option<String>,
    pub(super) status: Option<String>,
    pub(super) disable: bool,
    pub(super) continuation: Option<String>,
}

impl RalphRuntimeState {
    pub(super) fn new() -> Self {
        Self {
            current: None,
            queued_turn_inputs: VecDeque::new(),
            toggle_pending: false,
            pending_continuation: None,
            cwd: PathBuf::new(),
            prompt_path_override: None,
            done_marker_override: None,
            blocked_marker_override: None,
        }
    }

    pub(super) fn configure_options(
        &mut self,
        cwd: PathBuf,
        prompt_path_override: Option<String>,
        done_marker_override: Option<String>,
        blocked_marker_override: Option<String>,
    ) {
        self.cwd = cwd;
        self.prompt_path_override = prompt_path_override;
        self.done_marker_override = done_marker_override;
        self.blocked_marker_override = blocked_marker_override;
    }

    pub(super) fn is_enabled(&self) -> bool {
        self.current.is_some()
    }

    #[cfg(test)]
    pub(super) fn state(&self) -> Option<&RalphState> {
        self.current.as_ref()
    }

    pub(super) fn enable(&mut self, config: RalphConfig) -> String {
        let prompt_path = config.prompt_path.display().to_string();
        self.current = Some(RalphState::new(config));
        self.toggle_pending = false;
        format!("ralph on ({prompt_path})")
    }

    pub(super) fn disable(&mut self) -> &'static str {
        self.current = None;
        self.toggle_pending = false;
        self.pending_continuation = None;
        self.queued_turn_inputs.clear();
        "ralph off"
    }

    pub(super) fn queue_initial_prompt(&mut self) {
        let Some(ralph) = self.current.as_mut() else {
            return;
        };
        if ralph.primed || ralph.completed {
            return;
        }
        ralph.primed = true;
        let prompt = ralph.config.base_prompt.clone();
        self.enqueue_turn_input(prompt, false);
    }

    pub(super) fn toggle_now(&mut self) -> Result<&'static str> {
        if self.is_enabled() {
            self.disable();
            return Ok("ralph off");
        }

        let cfg = load_ralph_config(
            &self.cwd,
            self.prompt_path_override.as_deref(),
            self.done_marker_override.as_deref(),
            self.blocked_marker_override.as_deref(),
        )?;
        self.enable(cfg);
        self.queue_initial_prompt();
        Ok("ralph on")
    }

    pub(super) fn request_toggle(&mut self, turn_active: bool) -> Result<&'static str> {
        if turn_active {
            self.toggle_pending = !self.toggle_pending;
            if self.toggle_pending {
                return Ok("ralph toggle pending");
            }
            return Ok("ralph toggle canceled");
        }
        self.toggle_now()
    }

    pub(super) fn apply_pending_toggle(&mut self) -> Result<Option<&'static str>> {
        if !self.toggle_pending {
            return Ok(None);
        }
        self.toggle_pending = false;
        self.toggle_now().map(Some)
    }

    #[cfg(test)]
    pub(super) fn toggle_pending(&self) -> bool {
        self.toggle_pending
    }

    pub(super) fn enqueue_turn_input(
        &mut self,
        text: impl Into<String>,
        record_input_history: bool,
    ) {
        let text = text.into();
        if text.trim().is_empty() {
            return;
        }
        self.queued_turn_inputs.push_back(QueuedTurnInput {
            text,
            record_input_history,
            keeps_ralph_pending: false,
            available_at: None,
        });
    }

    pub(super) fn queue_continuation(&mut self, text: impl Into<String>) {
        let text = text.into();
        if text.trim().is_empty() {
            return;
        }
        self.pending_continuation = Some(QueuedTurnInput {
            text,
            record_input_history: false,
            keeps_ralph_pending: true,
            available_at: Some(Instant::now() + RALPH_CONTINUATION_DELAY),
        });
    }

    pub(super) fn has_pending_continuation(&self) -> bool {
        self.pending_continuation.is_some()
            || self
                .queued_turn_inputs
                .iter()
                .any(|turn| turn.keeps_ralph_pending)
    }

    pub(super) fn pending_continuation_wait(&self, now: Instant) -> Option<Duration> {
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
    pub(super) fn pending_continuation_deadline(&self) -> Option<Instant> {
        self.pending_continuation
            .as_ref()
            .and_then(|turn| turn.available_at)
    }

    pub(super) fn promote_ready_continuation(&mut self, now: Instant) {
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

    pub(super) fn dequeue_turn_input(&mut self, now: Instant) -> Option<QueuedTurnInput> {
        self.promote_ready_continuation(now);
        if let Some(turn) = self.queued_turn_inputs.pop_front() {
            return Some(turn);
        }
        None
    }

    #[cfg(test)]
    pub(super) fn queued_turn_inputs_is_empty(&self) -> bool {
        self.queued_turn_inputs.is_empty()
    }

    pub(super) fn has_ready_queued_turn_input(&self, now: Instant) -> bool {
        if !self.queued_turn_inputs.is_empty() {
            return true;
        }
        self.pending_continuation
            .as_ref()
            .and_then(|turn| turn.available_at)
            .is_some_and(|deadline| now >= deadline)
    }

    pub(super) fn mark_user_turn_submitted(&mut self) {
        if let Some(ralph) = self.current.as_mut() {
            ralph.waiting_for_user = false;
        }
    }

    pub(super) fn blocked_marker_outcome(
        &self,
        messages: &[Message],
        start_idx: usize,
    ) -> Option<RalphTurnOutcome> {
        let blocked_marker = self.current.as_ref()?.config.blocked_marker.clone();
        let markers = detect_turn_markers(messages, start_idx, "", &blocked_marker);
        if !markers.blocked {
            return None;
        }
        Some(RalphTurnOutcome {
            system_message: Some("Ralph blocked: waiting for input".to_string()),
            status: Some("ralph blocked".to_string()),
            disable: true,
            continuation: None,
        })
    }

    pub(super) fn handle_turn_completed(
        &mut self,
        messages: &[Message],
        start_idx: usize,
        interrupted: bool,
    ) -> RalphTurnOutcome {
        let Some(ralph) = self.current.as_mut() else {
            return RalphTurnOutcome {
                system_message: None,
                status: None,
                disable: false,
                continuation: None,
            };
        };

        let markers = detect_turn_markers(
            messages,
            start_idx,
            &ralph.config.done_marker,
            &ralph.config.blocked_marker,
        );
        if markers.completed {
            ralph.completed = true;
            ralph.waiting_for_user = false;
            return RalphTurnOutcome {
                system_message: Some("Ralph complete".to_string()),
                status: Some("ralph complete".to_string()),
                disable: true,
                continuation: None,
            };
        }
        if markers.blocked {
            ralph.waiting_for_user = false;
            return RalphTurnOutcome {
                system_message: Some("Ralph blocked: waiting for input".to_string()),
                status: Some("ralph blocked".to_string()),
                disable: true,
                continuation: None,
            };
        }
        if interrupted {
            ralph.waiting_for_user = false;
            return RalphTurnOutcome {
                system_message: None,
                status: None,
                disable: false,
                continuation: None,
            };
        }
        if !ralph.completed {
            ralph.waiting_for_user = false;
            return RalphTurnOutcome {
                system_message: None,
                status: Some("ralph continuing".to_string()),
                disable: false,
                continuation: Some(ralph.config.continuation_prompt.clone()),
            };
        }

        RalphTurnOutcome {
            system_message: None,
            status: None,
            disable: false,
            continuation: None,
        }
    }
}
