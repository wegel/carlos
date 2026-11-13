//! Keep typed native callback identities and independent answer drafts for Codex.

use std::collections::{HashMap, HashSet};

use carlos_core::session::codex::callbacks::CallbackKey;

use super::claude_question_state::NativeDraft;

/// Keep only local editors and which callback this terminal opened.
#[derive(Default)]
pub(super) struct CodexCallbacksView {
    pub(super) drafts: HashMap<CallbackKey, NativeDraft>,
    pub(super) focused: bool,
    pub(super) opened: HashSet<CallbackKey>,
}
