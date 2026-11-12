//! Keep typed native callback identities and independent answer drafts for Codex.

use std::collections::{HashMap, HashSet};

use super::claude_question_state::NativeDraft;
use crate::session::codex::callbacks::CallbackKey;

/// One native process owns its close fence and pending callback lifetimes.
#[derive(Default)]
pub(super) struct CodexUiState {
    pub(super) closed: bool,
    pub(super) item_turn: Option<String>,
    pub(super) item_part: usize,
    pub(super) connection: super::codex_reconnect_state::ConnectionState,
    pub(super) enabled: bool,
    pub(super) epoch: u64,
    pub(super) settings: super::codex_settings::SettingsState,
    pub(super) panel: super::codex_panel_state::PanelState,
    pub(super) goals: super::codex_goal_state::GoalState,
    pub(super) task_status: super::codex_task_status::TaskStatus,
}

/// Keep only local editors and which callback this terminal opened.
#[derive(Default)]
pub(super) struct CodexCallbacksView {
    pub(super) drafts: HashMap<CallbackKey, NativeDraft>,
    pub(super) focused: bool,
    pub(super) opened: HashSet<CallbackKey>,
}
