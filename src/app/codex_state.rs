//! Keep typed native callback identities and independent answer drafts for Codex.

use std::collections::{HashMap, HashSet};

use super::claude_question_state::NativeDraft;
use crate::session::codex::callbacks::CallbackKey;

/// One native process owns its close fence and pending callback lifetimes.
#[derive(Default)]
pub(super) struct CodexUiState {
    pub(super) connection: super::codex_reconnect_state::ConnectionState,
}

/// Keep only local editors and which callback this terminal opened.
#[derive(Default)]
pub(super) struct CodexCallbacksView {
    pub(super) drafts: HashMap<CallbackKey, NativeDraft>,
    pub(super) focused: bool,
    pub(super) opened: HashSet<CallbackKey>,
}
