//! Retire callbacks and fence submitted candidates without touching local editors.

use serde_json::Value;

use super::callbacks::CallbackKey;
use crate::session::approval_state::ApprovalQueue;
use crate::session::entry_id::EntryId;
use crate::session::state::SessionState;

/// Tell the client exactly which editor and consent hints to discard.
pub(crate) struct CallbackRetired {
    pub(crate) key: Option<CallbackKey>,
    pub(crate) active: bool,
    pub(crate) cards: Vec<EntryId>,
}

/// Describe only the local changes caused by a form worker reply.
#[derive(Default)]
pub(crate) struct CallbackChecked {
    pub(crate) clear_notice: bool,
    pub(crate) unsupported: Option<String>,
    pub(crate) error: Option<String>,
    pub(crate) reply: Option<(EntryId, Value)>,
}

impl SessionState {
    /// Match native resolution by exact identity and owning thread, even before focus.
    pub(crate) fn resolve_codex_callback(&mut self, params: &Value) -> Option<CallbackRetired> {
        let id = &params["requestId"];
        if let Some(entry) = self.codex_callbacks.get(id) {
            if entry.params["threadId"] != params["threadId"] {
                return None;
            }
        }
        Some(self.retire_codex_callback(id))
    }

    /// Remove this callback's queued card, then promote only when it was active.
    pub(crate) fn retire_codex_callback(&mut self, id: &Value) -> CallbackRetired {
        let active = self
            .approval()
            .pending()
            .is_some_and(|p| p.facts().request_id == *id);
        let mut cards = self.retire_queued_approvals(id, ApprovalQueue::Codex);
        let key = self.codex_callbacks.retire(id);
        if active {
            cards.extend(self.promote_approval(ApprovalQueue::Codex));
        }
        CallbackRetired { key, active, cards }
    }

    /// List the exact live identities for the existing per-request reset path.
    pub(crate) fn codex_callback_ids(&self) -> Vec<Value> {
        self.codex_callbacks
            .entries
            .keys()
            .map(CallbackKey::id)
            .collect()
    }

    /// Forget the previous native process, including its bounded retired-id window.
    pub(crate) fn clear_codex_callbacks(&mut self) {
        self.codex_callbacks = Default::default();
    }

    /// Invalidate the submitted candidate when this callback's local editor changes.
    pub(crate) fn edit_codex_callback(&mut self, id: &Value) {
        if let Some(entry) = self.codex_callbacks.get_mut(id) {
            entry.revision = entry.revision.saturating_add(1);
            entry.candidate = None;
            entry.candidate_card = None;
            entry.checking = false;
        }
    }

    /// Clear a failed worker's busy flag while retaining its submitted candidate.
    pub(crate) fn codex_callback_reply_failed(&mut self, id: &Value) {
        if let Some(entry) = self.codex_callbacks.get_mut(id) {
            entry.checking = false;
        }
    }

    /// Consume a worker reply only for a live callback and its current answer revision.
    pub(crate) fn check_codex_callback(&mut self, params: &Value) -> Option<CallbackChecked> {
        let entry = self.codex_callbacks.get_mut(&params["requestId"])?;
        let revision = params["revision"].as_u64()?;
        let mut update = CallbackChecked::default();
        if revision == 0 {
            update.clear_notice = !entry.checking;
            if params["ok"] != true {
                update.unsupported = Some(crate::session::text::safe_text(
                    params["error"]
                        .as_str()
                        .unwrap_or("Unsupported form schema"),
                ));
            }
            return Some(update);
        }
        if revision != entry.revision {
            return None;
        }
        entry.checking = false;
        update.clear_notice = true;
        if params["ok"] != true {
            update.error = Some(crate::session::text::safe_text(
                params["error"].as_str().unwrap_or("Form validation failed"),
            ));
            entry.candidate = None;
            entry.candidate_card = None;
            return Some(update);
        }
        // Keep the old take order: a missing candidate does not consume the card token.
        if let Some(result) = entry.candidate.take() {
            if let Some(card_id) = entry.candidate_card.take() {
                update.reply = Some((card_id, result));
            }
        }
        Some(update)
    }
}
