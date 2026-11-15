//! Keep consent keys and their card hints in the terminal that received them.

use std::collections::HashMap;

use carlos_core::session::approval_state::PendingApprovalRequest;
use carlos_core::session::entry_id::EntryId;

use super::claude_question_view::default_no_hint;
use super::state::AppState;

/// Keep each card's local hint text while backend requests retain their details.
#[derive(Default)]
pub(super) struct ApprovalHints {
    cards: HashMap<EntryId, ApprovalCardView>,
}

#[derive(Default)]
struct ApprovalCardView {
    hints: Vec<String>,
    claude_fields: Option<bool>,
}

impl ApprovalHints {
    /// Remember how this callback was first shown, even if a helper drops its editor.
    pub(super) fn register_callback(&mut self, entry: EntryId, has_fields: bool) {
        self.cards.entry(entry).or_default().claude_fields = Some(has_fields);
    }

    /// Reset added hint lines when the session keeps a duplicate card.
    pub(super) fn clear_extra_lines(&mut self, entry: Option<EntryId>) {
        if let Some(card) = entry.and_then(|id| self.cards.get_mut(&id)) {
            card.hints.clear();
        }
    }
}

impl AppState {
    /// Drop hints for the cards retired by a session command.
    pub(super) fn drop_retired_approval_hints(&mut self, retired: Vec<EntryId>) {
        for id in retired {
            self.view.approval_hints.cards.remove(&id);
        }
    }

    /// Prepend the same hint as HEAD, including repeated consent after focus changes.
    pub(super) fn prepend_approval_hint(&mut self, hint: &str) {
        if let Some(entry) = self.session.approval().pending().map(|p| p.entry_id()) {
            self.view
                .approval_hints
                .cards
                .entry(entry)
                .or_default()
                .hints
                .insert(0, hint.into());
        }
    }

    /// Read local card hints before the backend's unchanged details.
    pub(super) fn approval_detail_lines<'a>(
        &'a self,
        pending: &'a PendingApprovalRequest,
    ) -> impl Iterator<Item = &'a str> {
        let payload = pending.facts().requested_permissions.as_ref();
        let card = self.view.approval_hints.cards.get(&pending.entry_id());
        let fields = card.and_then(|card| card.claude_fields);
        let native = pending.facts().method == "claude/native" && fields.is_some();
        let browser = native && payload.is_some_and(|p| p["url"].is_string());
        let codex_browser = pending.facts().method == "mcpServer/elicitation/request"
            && pending
                .facts()
                .detail_lines
                .first()
                .is_some_and(|line| line.starts_with("URL host: "));
        let default_no = native && payload.is_some_and(|p| p["defaultToNo"] == true);
        let has_fields = fields.unwrap_or(false);
        let nonblocking = pending.facts().method == "item/tool/requestUserInput"
            && payload.is_some_and(|p| p["isBlocking"] == false);
        let last = pending.facts().detail_lines.len().saturating_sub(1);
        let details = pending
            .facts()
            .detail_lines
            .iter()
            .enumerate()
            .map(move |(index, line)| {
                if nonblocking && index == last {
                    "Nonblocking question; Esc returns to your draft"
                } else {
                    line.as_str()
                }
            });
        let prefix = if browser || codex_browser { 2 } else { 0 };
        card.into_iter()
            .flat_map(|card| card.hints.iter())
            .map(String::as_str)
            .chain(details.clone().take(prefix))
            .chain(
                browser.then_some("Ctrl+Y copies the link; y consents to this server's request."),
            )
            .chain(codex_browser.then_some(
                "Ctrl+Y copies the link; accept consents without confirming completion.",
            ))
            .chain(default_no.then_some(default_no_hint(has_fields, false)))
            .chain(details.skip(prefix))
            .chain(browser.then_some(
                "Accept consents to this browser interaction; it does not confirm completion.",
            ))
    }
}
