//! Keep typed native callback identities and independent answer drafts for Codex.

use std::collections::{HashMap, HashSet, VecDeque};

use serde_json::Value;

use super::claude_question_state::NativeDraft;
use super::state::PendingApprovalRequest;

/// One native process owns its close fence and pending callback lifetimes.
#[derive(Default)]
pub(super) struct CodexUiState {
    pub(super) closed: bool,
    pub(super) item_turn: Option<String>,
    pub(super) item_part: usize,
    pub(super) connection: super::codex_reconnect_state::ConnectionState,
    pub(super) enabled: bool,
    pub(super) callbacks: CodexCallbacks,
    pub(super) queue: super::codex_queue_state::QueueState,
    pub(super) epoch: u64,
    pub(super) serial: u64,
    pub(super) settings: super::codex_settings::SettingsState,
    pub(super) panel: super::codex_panel_state::PanelState,
    pub(super) goals: super::codex_goal_state::GoalState,
    pub(super) task_status: super::codex_task_status::TaskStatus,
}

/// Retain callback drafts independently of root-turn activity and overlay focus.
#[derive(Default)]
pub(super) struct CodexCallbacks {
    pub(super) queue: VecDeque<PendingApprovalRequest>,
    pub(super) entries: HashMap<String, CodexCallback>,
    pub(super) focused: bool,
    pub(super) opened: HashSet<String>,
    recent: VecDeque<String>,
    retired: HashSet<String>,
}

/// Hold native payloads separately from the shared answer editor and Claude wire.
pub(super) struct CodexCallback {
    pub(super) draft: NativeDraft,
    pub(super) params: Value,
    pub(super) blocking: bool,
    pub(super) revision: u64,
    pub(super) candidate: Option<Value>,
    pub(super) checking: bool,
    pub(super) interrupting: bool,
    pub(super) bytes: usize,
}

impl CodexCallbacks {
    /// Distinguish native string and numeric IDs without deriving identity from prose.
    pub(super) fn key(id: &Value) -> Option<String> {
        if !(id.is_string() || id.is_number()) || id.as_str().is_some_and(|text| text.len() > 4096)
        {
            return None;
        }
        let key = id.to_string();
        (key.len() <= 4096).then_some(key)
    }

    /// Detect active duplicates and bounded recent replays from this native process.
    pub(super) fn contains(&self, id: &Value) -> bool {
        Self::key(id)
            .is_some_and(|key| self.entries.contains_key(&key) || self.retired.contains(&key))
    }

    /// Retire only the exact typed request; old IDs use a bounded replay window.
    pub(super) fn retire(&mut self, id: &Value) {
        let Some(key) = Self::key(id) else {
            return;
        };
        self.entries.remove(&key);
        self.opened.remove(&key);
        self.queue.retain(|pending| pending.request_id != *id);
        if self.retired.insert(key.clone()) {
            self.recent.push_back(key);
        }
        while self.recent.len() > 2048 {
            if let Some(id) = self.recent.pop_front() {
                self.retired.remove(&id);
            }
        }
    }

    /// Find a callback with its original typed RPC ID.
    pub(super) fn get(&self, id: &Value) -> Option<&CodexCallback> {
        self.entries.get(&Self::key(id)?)
    }

    /// Keep edits tied to their native callback rather than the visible composer.
    pub(super) fn get_mut(&mut self, id: &Value) -> Option<&mut CodexCallback> {
        self.entries.get_mut(&Self::key(id)?)
    }
}
