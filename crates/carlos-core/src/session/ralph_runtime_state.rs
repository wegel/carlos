//! Runtime lifecycle for Ralph mode: enable/disable and turn-marker decisions.

#[path = "ralph_turn_queue.rs"]
mod ralph_turn_queue;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;
use carlos_proto::submissions::{ClientOperation, SubmittedText};

use crate::backend::PromptOrigin;

use super::ralph::{detect_turn_markers, load_ralph_config, RalphConfig, RalphState};
use crate::session::models::Message;

// --- Continuation Delay ---

const RALPH_CONTINUATION_DELAY: Duration = Duration::from_millis(700);

// --- Turn Queue ---

/// Queuedturninput.
#[cfg_attr(any(test, feature = "test-support"), derive(Debug))]
pub struct QueuedTurnInput {
    pub text: String,
    pub record_input_history: bool,
    pub origin: PromptOrigin,
    pub operation: Option<ClientOperation>,
    keeps_ralph_pending: bool,
    available_at: Option<Instant>,
}

// --- Runtime State ---

/// Ralphruntimestate.
#[cfg_attr(any(test, feature = "test-support"), derive(Debug))]
pub struct RalphRuntimeState {
    current: Option<RalphState>,
    queued_turn_inputs: VecDeque<QueuedTurnInput>,
    toggle_pending: bool,
    pending_continuation: Option<QueuedTurnInput>,
    cwd: PathBuf,
    prompt_path_override: Option<String>,
    done_marker_override: Option<String>,
    blocked_marker_override: Option<String>,
    waiting_marker_override: Option<String>,
}

// --- Turn Outcome ---

#[derive(Default)]
/// Ralphturnoutcome.
pub(crate) struct RalphTurnOutcome {
    pub(crate) system_message: Option<String>,
    pub status: Option<String>,
    pub(crate) disable: bool,
    pub continuation: Option<String>,
}

impl RalphTurnOutcome {
    /// Nothing to report (idle or interrupted with no marker hit).
    fn idle() -> Self {
        Self::default()
    }

    /// Ralph reached the done marker. Disables ralph mode.
    fn complete() -> Self {
        Self {
            system_message: Some("Ralph complete".to_string()),
            status: Some("ralph complete".to_string()),
            disable: true,
            continuation: None,
        }
    }

    /// Ralph reached the blocked marker. Disables ralph mode.
    fn blocked() -> Self {
        Self {
            system_message: Some("Ralph blocked: waiting for input".to_string()),
            status: Some("ralph blocked".to_string()),
            disable: true,
            continuation: None,
        }
    }

    /// Ralph reached a human gate. Disable Ralph and wait for input.
    fn waiting() -> Self {
        Self {
            system_message: Some("Ralph waiting for human input".to_string()),
            status: Some("ralph waiting".to_string()),
            disable: true,
            continuation: None,
        }
    }

    /// Ralph turn finished without a marker. Queue the continuation prompt.
    fn continuing(prompt: String) -> Self {
        Self {
            system_message: None,
            status: Some("ralph continuing".to_string()),
            disable: false,
            continuation: Some(prompt),
        }
    }
}

// --- Ralph Logic ---

impl Default for RalphRuntimeState {
    fn default() -> Self {
        Self::new()
    }
}

impl RalphRuntimeState {
    /// New.
    pub fn new() -> Self {
        Self {
            current: None,
            queued_turn_inputs: VecDeque::new(),
            toggle_pending: false,
            pending_continuation: None,
            cwd: PathBuf::new(),
            prompt_path_override: None,
            done_marker_override: None,
            blocked_marker_override: None,
            waiting_marker_override: None,
        }
    }

    /// Configure options.
    pub(crate) fn configure_options(
        &mut self,
        cwd: PathBuf,
        prompt_path_override: Option<String>,
        done_marker_override: Option<String>,
        blocked_marker_override: Option<String>,
        waiting_marker_override: Option<String>,
    ) {
        self.cwd = cwd;
        self.prompt_path_override = prompt_path_override;
        self.done_marker_override = done_marker_override;
        self.blocked_marker_override = blocked_marker_override;
        self.waiting_marker_override = waiting_marker_override;
    }

    /// Is enabled.
    pub(crate) fn is_enabled(&self) -> bool {
        self.current.is_some()
    }

    #[cfg(any(test, feature = "test-support"))]
    /// State.
    pub fn state(&self) -> Option<&RalphState> {
        self.current.as_ref()
    }

    /// Enable.
    pub fn enable(&mut self, config: RalphConfig) -> String {
        let prompt_path = config.prompt_path.display().to_string();
        self.current = Some(RalphState::new(config));
        self.toggle_pending = false;
        format!("ralph on ({prompt_path})")
    }

    /// Disable.
    pub(crate) fn disable(&mut self) -> &'static str {
        self.current = None;
        self.toggle_pending = false;
        self.pending_continuation = None;
        self.queued_turn_inputs.clear();
        "ralph off"
    }

    /// Stop automatic Ralph work while retaining queued human input.
    pub(crate) fn stop_after_failure(&mut self) {
        self.current = None;
        self.toggle_pending = false;
        self.pending_continuation = None;
        self.queued_turn_inputs
            .retain(|turn| turn.origin == PromptOrigin::Human);
    }

    /// Return unsent human drafts after a terminal backend failure.
    pub(crate) fn take_queued_human_drafts(&mut self) -> Vec<SubmittedText> {
        self.queued_turn_inputs
            .drain(..)
            .filter(|turn| turn.origin == PromptOrigin::Human)
            .map(|turn| SubmittedText {
                text: turn.text,
                operation: turn.operation,
            })
            .collect()
    }

    /// Queue initial prompt.
    pub(crate) fn queue_initial_prompt(&mut self) {
        let Some(ralph) = self.current.as_mut() else {
            return;
        };
        if ralph.primed || ralph.completed {
            return;
        }
        ralph.primed = true;
        let prompt = ralph.config.base_prompt.clone();
        self.enqueue_prompt(prompt, false, PromptOrigin::Automatic);
    }

    pub(in crate::session) fn prepare_config(
        &self,
    ) -> impl FnOnce() -> Result<RalphConfig> + Send + 'static {
        let cwd = self.cwd.clone();
        let path = self.prompt_path_override.clone();
        let done = self.done_marker_override.clone();
        let blocked = self.blocked_marker_override.clone();
        let waiting = self.waiting_marker_override.clone();
        move || {
            load_ralph_config(
                &cwd,
                path.as_deref(),
                done.as_deref(),
                blocked.as_deref(),
                waiting.as_deref(),
            )
        }
    }

    /// Toggle now.
    pub(crate) fn toggle_now(&mut self) -> Result<&'static str> {
        if self.is_enabled() {
            self.disable();
            return Ok("ralph off");
        }

        let cfg = load_ralph_config(
            &self.cwd,
            self.prompt_path_override.as_deref(),
            self.done_marker_override.as_deref(),
            self.blocked_marker_override.as_deref(),
            self.waiting_marker_override.as_deref(),
        )?;
        self.enable(cfg);
        self.queue_initial_prompt();
        Ok("ralph on")
    }

    /// Request toggle.
    pub(crate) fn request_toggle(&mut self, turn_active: bool) -> Result<&'static str> {
        if turn_active {
            self.toggle_pending = !self.toggle_pending;
            if self.toggle_pending {
                return Ok("ralph toggle pending");
            }
            return Ok("ralph toggle canceled");
        }
        self.toggle_now()
    }

    /// Apply pending toggle.
    pub(crate) fn apply_pending_toggle(&mut self) -> Result<Option<&'static str>> {
        if !self.toggle_pending {
            return Ok(None);
        }
        self.toggle_pending = false;
        self.toggle_now().map(Some)
    }

    /// Check a requested toggle before another automatic driver can start.
    pub fn toggle_pending(&self) -> bool {
        self.toggle_pending
    }

    /// Mark user turn submitted.
    pub fn mark_user_turn_submitted(&mut self) {
        if let Some(ralph) = self.current.as_mut() {
            ralph.waiting_for_user = false;
        }
    }

    /// Blocked marker outcome.
    pub(crate) fn blocked_marker_outcome(
        &self,
        messages: &[Message],
        start_idx: usize,
    ) -> Option<RalphTurnOutcome> {
        let blocked_marker = self.current.as_ref()?.config.blocked_marker.clone();
        let markers = detect_turn_markers(messages, start_idx, "", &blocked_marker, "");
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

    /// Handle turn completed.
    pub(crate) fn handle_turn_completed(
        &mut self,
        messages: &[Message],
        start_idx: usize,
        interrupted: bool,
    ) -> RalphTurnOutcome {
        let Some(ralph) = self.current.as_mut() else {
            return RalphTurnOutcome::idle();
        };

        let markers = detect_turn_markers(
            messages,
            start_idx,
            &ralph.config.done_marker,
            &ralph.config.blocked_marker,
            &ralph.config.waiting_marker,
        );
        if markers.completed {
            ralph.completed = true;
            ralph.waiting_for_user = false;
            return RalphTurnOutcome::complete();
        }
        if markers.blocked {
            ralph.waiting_for_user = false;
            return RalphTurnOutcome::blocked();
        }
        if markers.waiting {
            ralph.waiting_for_user = false;
            return RalphTurnOutcome::waiting();
        }
        if interrupted {
            ralph.waiting_for_user = false;
            return RalphTurnOutcome::idle();
        }
        if !ralph.completed {
            ralph.waiting_for_user = false;
            return RalphTurnOutcome::continuing(ralph.config.continuation_prompt.clone());
        }
        RalphTurnOutcome::idle()
    }
}
