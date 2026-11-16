//! Retire the active approval and report the exact client editor to discard.

use serde_json::Value;

use super::approval_state::{ApprovalQueue, ApprovalRequestKind};
use super::codex::callback_commands::CallbackRetired;
use super::entry_id::EntryId;
use super::state::SessionState;

/// Retire session cards without retaining any client's answer draft.
pub enum ApprovalCleared {
    Codex(CallbackRetired),
    Other {
        editor: Option<String>,
        cards: Vec<EntryId>,
    },
}

impl SessionState {
    /// Preserve Codex retirement precedence, otherwise promote the Claude queue.
    pub fn clear_pending_session_approval(&mut self) -> ApprovalCleared {
        let pending_id = self
            .approval()
            .pending()
            .map(|p| p.facts().request_id.clone());
        if let Some(id) = pending_id
            .as_ref()
            .filter(|id| self.codex_callbacks().get(id).is_some())
        {
            return ApprovalCleared::Codex(self.retire_codex_callback(id));
        }
        let editor = pending_id
            .as_ref()
            .and_then(Value::as_str)
            .map(str::to_string);
        let queue = if self
            .approval()
            .pending()
            .is_some_and(|card| card.facts().kind == ApprovalRequestKind::GrokPermission)
        {
            ApprovalQueue::Grok
        } else {
            ApprovalQueue::Claude
        };
        let cards = self.promote_approval(queue);
        ApprovalCleared::Other { editor, cards }
    }

    /// Withdraw the exact browser request and queued cards before promoting an active one.
    pub(crate) fn cancel_claude_callback(
        &mut self,
        id: &Value,
    ) -> (Vec<EntryId>, Option<ApprovalCleared>) {
        if let Some(id) = id.as_str() {
            self.withdraw_claude_browser(id);
        }
        let retired = self.retire_queued_approvals(id, ApprovalQueue::Claude);
        let cleared = self
            .approval()
            .pending()
            .is_some_and(|p| &p.facts().request_id == id)
            .then(|| self.clear_pending_session_approval());
        (retired, cleared)
    }
}
