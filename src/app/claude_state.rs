//! Tracks Claude prompt UUIDs across native result cycles and session-state events.

use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};

use crate::claude_backend::ClaudeTranslationState;

use super::claude_question_state::NativeAnswers;
use super::state::PendingApprovalRequest;

/// Distinguishes a query still starting from a helper that has closed permanently.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ClaudePhase {
    Starting,
    Ready,
    Closed,
}

/// Tracks native Claude messages and unanswered questions for one UI session.
pub(super) struct ClaudeUiState {
    pub(super) phase: ClaudePhase,
    pub(super) enabled: bool,
    pub(super) epoch: u64,
    pub(super) children: super::claude_children::ClaudeChildren,
    pub(super) notices: VecDeque<String>,
    pub(super) quota: super::claude_quota::ClaudeQuota,
    pub(super) reset_key: Option<String>,
    pub(super) awaiting_reset_init: bool,
    pub(super) tasks: super::claude_tasks::ClaudeTasks,
    pub(super) activity: super::claude_activity::ClaudeActivity,
    pub(super) prompts: super::claude_prompts::ClaudePrompts,
    pub(super) generation: u64,
    pub(super) history: super::claude_session_history::ClaudeHistory,
    pub(super) capabilities: Option<Value>,
    pub(super) observed_model: Option<String>,
    pub(super) translation: ClaudeTranslationState,
    pub(super) questions: VecDeque<PendingApprovalRequest>,
    pub(super) confirm_allow: Option<String>,
    pub(super) answers: NativeAnswers,
    pub(super) selection: ClaudeSelection,
    pending: VecDeque<String>,
    cycle: Vec<String>,
    results: Vec<(String, &'static str)>,
    interrupted: HashSet<String>,
    idle: bool,
}

/// Separates native defaults from explicit Carlos model and effort choices.
pub(super) struct ClaudeSelection {
    pub(super) inherits_model: bool,
    pub(super) inherits_effort: bool,
    pub(super) editing: Option<(String, String)>,
}

impl Default for ClaudeSelection {
    fn default() -> Self {
        Self {
            inherits_model: true,
            inherits_effort: true,
            editing: None,
        }
    }
}

impl Default for ClaudeUiState {
    fn default() -> Self {
        static STREAM: AtomicU64 = AtomicU64::new(1);
        let mut translation = ClaudeTranslationState::default();
        translation.stream_instance_seq = STREAM.fetch_add(1, Ordering::Relaxed);
        Self {
            phase: ClaudePhase::Ready,
            enabled: false,
            epoch: 0,
            children: Default::default(),
            notices: Default::default(),
            quota: Default::default(),
            reset_key: None,
            awaiting_reset_init: false,
            tasks: Default::default(),
            activity: Default::default(),
            prompts: Default::default(),
            generation: 1,
            history: super::claude_session_history::ClaudeHistory::default(),
            capabilities: None,
            observed_model: None,
            translation,
            questions: VecDeque::new(),
            confirm_allow: None,
            answers: NativeAnswers::default(),
            selection: ClaudeSelection::default(),
            pending: VecDeque::new(),
            cycle: Vec::new(),
            results: Vec::new(),
            interrupted: HashSet::new(),
            idle: false,
        }
    }
}

impl ClaudeUiState {
    /// Remember a prompt before its native stream can arrive.
    pub(super) fn submitted(&mut self, id: &str) {
        if !self.pending.iter().any(|pending| pending == id) {
            self.pending.push_back(id.to_string());
        }
        self.idle = false;
    }

    /// Remove a prompt that the helper did not accept.
    pub(super) fn rejected(&mut self, id: &str) {
        self.pending.retain(|pending| pending != id);
    }

    /// Bind a native reply to every prompt UUID it names.
    pub(super) fn bind_reply(&mut self, message: &Value) {
        let ids = prompt_ids(message);
        if message["type"] == "system" && message["subtype"] == "init" {
            self.cycle.clear();
            self.translation.current_turn_id = None;
        }
        if !ids.is_empty() && ids != self.cycle {
            self.idle = false;
            self.cycle = ids;
        }
        if let Some(id) = self.cycle.last() {
            self.translation.current_turn_id = Some(id.clone());
        }
    }

    /// Capture the current prompt IDs before sending an interrupt.
    pub(super) fn begin_interrupt(&mut self) -> Vec<String> {
        let mut ids = self.cycle.clone();
        if ids.is_empty() {
            ids = self
                .translation
                .current_turn_id
                .clone()
                .into_iter()
                .collect();
        }
        self.interrupted.extend(ids.iter().cloned());
        ids
    }

    /// Undo captured interrupt intent when the control request fails.
    pub(super) fn rollback_interrupt(&mut self, ids: &[String]) {
        for id in ids {
            self.interrupted.remove(id);
        }
    }

    /// Store terminal results until the native session also reports idle.
    pub(super) fn result(&mut self, message: &Value) {
        let mut ids = prompt_ids(message);
        if ids.is_empty() {
            ids = self.cycle.clone();
        }
        for id in ids {
            if !self.pending.contains(&id) {
                continue;
            }
            if self
                .prompts
                .entries
                .get(&id)
                .is_some_and(|prompt| prompt.clear)
            {
                self.prompts.entries.remove(&id);
                self.pending.retain(|pending| pending != &id);
                continue;
            }
            let requested = self.interrupted.remove(&id);
            let status = if matches!(
                message["terminal_reason"].as_str(),
                Some("aborted_streaming" | "aborted_tools")
            ) {
                "interrupted"
            } else if message["is_error"] == true || message["subtype"] != "success" {
                "failed"
            } else if requested {
                "interrupted"
            } else {
                "completed"
            };
            if !self.results.iter().any(|(existing, _)| existing == &id) {
                self.results.push((id, status));
            }
        }
        self.translation.current_turn_id = None;
        self.cycle.clear();
    }

    /// Record whether the native main turn has stopped running.
    pub(super) fn set_idle(&mut self, idle: bool) {
        self.idle = idle;
    }

    /// Release prompt completions once both result and idle have arrived.
    pub(super) fn completions(&mut self) -> Vec<String> {
        if !self.idle {
            return Vec::new();
        }
        self.results.drain(..).map(|(id, status)| {
            self.pending.retain(|pending| pending != &id);
            self.prompts.entries.remove(&id);
            json!({ "method": "turn/completed", "params": { "turn": { "id": id, "status": status } } }).to_string()
        }).collect()
    }

    /// Clear consumed work while preserving accepted inputs that native Claude has not run yet.
    pub(super) fn reset_prompts(&mut self, clear: Option<&str>) {
        self.prompts
            .entries
            .retain(|id, prompt| !prompt.consumed || Some(id.as_str()) == clear);
        if let Some(prompt) = clear.and_then(|id| self.prompts.entries.get_mut(id)) {
            prompt.clear = true;
        }
        self.pending
            .retain(|id| self.prompts.entries.contains_key(id));
        self.cycle.clear();
        self.results.clear();
        self.interrupted.clear();
        self.idle = false;
    }

    /// Fail pending prompts when the helper session closes.
    pub(super) fn fail_pending(&mut self) -> Vec<String> {
        self.results.clear();
        self.results
            .extend(self.pending.drain(..).map(|id| (id, "failed")));
        self.idle = true;
        self.completions()
    }
}

/// Read only the exact prompt UUIDs that native Claude names as consumed.
pub(super) fn prompt_ids(message: &Value) -> Vec<String> {
    if let Some(ids) = message["user_message_uuids"].as_array() {
        return ids
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect();
    }
    message["user_message_uuid"]
        .as_str()
        .map(str::to_string)
        .into_iter()
        .collect()
}
