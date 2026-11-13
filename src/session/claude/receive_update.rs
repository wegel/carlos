//! Return immediate Claude editor effects without storing any client draft.

use serde_json::Value;

use super::callbacks::ClaudeCallback;
use crate::session::actions::ServerRequestAction;
use crate::session::approval_reply::ApprovalCleared;
use crate::session::codex::callback_commands::CallbackRetired;
use crate::session::entry_id::EntryId;
use crate::session::item_changes::ItemChanges;
use crate::session::thread_commands::ThreadChanged;

/// Borrow unsent client facts only for one synchronous command.
pub(crate) struct ClaudeDraftContext<'a> {
    pub(crate) composer: &'a [String],
    pub(crate) confirmation: Option<EntryId>,
}

/// Report only local changes and the existing next backend action.
#[derive(Default)]
pub(crate) struct ClaudeUpdate {
    pub(crate) items: ItemChanges,
    pub(crate) effects: Vec<ClaudeEffect>,
    pub(crate) action: Option<ServerRequestAction>,
    pub(crate) history_frame: Option<Value>,
}

/// Tell the synchronous client which editors or controls to change once.
pub(crate) enum ClaudeEffect {
    Callback(ClaudeCallback),
    Cancelled {
        id: Option<String>,
        clear_confirmation: bool,
    },
    Browser(Option<String>),
    Thread(ThreadChanged),
    HistoryReset,
    Cleared(ApprovalCleared),
    Cards(Vec<EntryId>),
    CodexRetired(CallbackRetired),
    CodexUnfocus,
    SwitchControls,
    ResetControls,
    EditFailed(bool),
    FollowBottom,
    SetInput(String),
    ClearInput,
    ClearRewind,
    RetainDraft(String),
    LoadRetained,
}

impl ClaudeUpdate {
    /// Read the composer after any earlier recovery in this synchronous command.
    pub(in crate::session) fn composer_is_empty(&self, draft: &ClaudeDraftContext<'_>) -> bool {
        match self.input_change() {
            Some(Some(text)) => text.is_empty(),
            Some(None) => true,
            None => {
                draft.composer.is_empty()
                    || draft.composer.len() == 1 && draft.composer[0].is_empty()
            }
        }
    }

    /// Compare borrowed composer lines without copying them into a new string.
    pub(in crate::session) fn composer_matches(
        &self,
        draft: &ClaudeDraftContext<'_>,
        text: &str,
    ) -> bool {
        match self.input_change() {
            Some(Some(current)) => current == text,
            Some(None) => text.is_empty(),
            None => text
                .split('\n')
                .eq(draft.composer.iter().map(String::as_str)),
        }
    }

    fn input_change(&self) -> Option<Option<&str>> {
        self.effects.iter().rev().find_map(|effect| match effect {
            ClaudeEffect::SetInput(text) => Some(Some(text.as_str())),
            ClaudeEffect::ClearInput => Some(None),
            _ => None,
        })
    }
}
