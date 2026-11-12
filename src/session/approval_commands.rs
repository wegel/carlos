//! Keep every issued approval card inside one private owner and one write point.

use std::collections::VecDeque;

use super::{ApprovalRequestDraft, PendingApprovalRequest};
use crate::session::entry_id::EntryId;
use crate::session::state::SessionState;

/// Select the backend's independently ordered callback queue.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ApprovalQueue {
    Claude,
    Codex,
}

/// Own every live card; removing a card retires its id forever.
#[derive(Debug, Default)]
pub(crate) struct ApprovalState {
    pending: Option<PendingApprovalRequest>,
    claude: VecDeque<PendingApprovalRequest>,
    codex: VecDeque<PendingApprovalRequest>,
}

impl ApprovalState {
    /// Borrow the active card without granting a setter or a copy of its identity.
    pub(crate) fn pending(&self) -> Option<&PendingApprovalRequest> {
        self.pending.as_ref()
    }

    /// Borrow the cards waiting for one backend.
    pub(crate) fn queued(&self, queue: ApprovalQueue) -> &VecDeque<PendingApprovalRequest> {
        match queue {
            ApprovalQueue::Claude => &self.claude,
            ApprovalQueue::Codex => &self.codex,
        }
    }

    /// Visit each live card once, in active, Claude, then Codex order.
    pub(crate) fn live(&self) -> impl Iterator<Item = &PendingApprovalRequest> {
        self.pending
            .iter()
            .chain(self.claude.iter())
            .chain(self.codex.iter())
    }
}

// Only this module can name a command or write the card containers.
enum ApprovalChange {
    Adopt(ApprovalRequestDraft, EntryId),
    Enqueue(ApprovalRequestDraft, EntryId, ApprovalQueue),
    Focus(EntryId, ApprovalQueue),
    Promote(ApprovalQueue),
    Retire(EntryId),
    ClearQueue(ApprovalQueue),
    BeginAnswer(EntryId),
}

impl ApprovalState {
    // This is the sole write point for the active card and both queues. Id-only
    // moves search the live containers. No retired-id ledger is needed: absent
    // ids cannot be inserted, and only the session allocator can mint a card.
    fn change(&mut self, change: ApprovalChange) {
        match change {
            ApprovalChange::Adopt(facts, entry_id) => {
                if self
                    .pending()
                    .is_some_and(|old| old.facts.same_request(&facts))
                {
                    return;
                }
                self.pending = Some(PendingApprovalRequest {
                    entry_id: Some(entry_id),
                    facts,
                    answer_submitted: false,
                });
            }
            ApprovalChange::Enqueue(facts, entry_id, queue) => {
                self.queue_mut(queue).push_back(PendingApprovalRequest {
                    entry_id: Some(entry_id),
                    facts,
                    answer_submitted: false,
                });
            }
            ApprovalChange::Focus(id, queue) => {
                let Some(index) = self
                    .queued(queue)
                    .iter()
                    .position(|p| p.entry_id() == Some(id))
                else {
                    return;
                };
                let card = self.queue_mut(queue).remove(index);
                if let Some(old) = self.pending.take() {
                    self.queue_mut(queue).push_back(old);
                }
                self.pending = card;
            }
            ApprovalChange::Promote(queue) => self.pending = self.queue_mut(queue).pop_front(),
            ApprovalChange::Retire(id) => {
                if self.pending().is_some_and(|p| p.entry_id() == Some(id)) {
                    self.pending = None;
                }
                self.claude.retain(|p| p.entry_id() != Some(id));
                self.codex.retain(|p| p.entry_id() != Some(id));
            }
            ApprovalChange::ClearQueue(queue) => self.queue_mut(queue).clear(),
            ApprovalChange::BeginAnswer(id) => {
                if let Some(card) = self.pending.as_mut().filter(|p| p.entry_id() == Some(id)) {
                    card.answer_submitted = true;
                }
            }
        }
    }

    fn queue_mut(&mut self, queue: ApprovalQueue) -> &mut VecDeque<PendingApprovalRequest> {
        match queue {
            ApprovalQueue::Claude => &mut self.claude,
            ApprovalQueue::Codex => &mut self.codex,
        }
    }
}

impl SessionState {
    /// Adopt new facts, preserving an identical active card's consent key.
    pub(crate) fn set_pending_approval(&mut self, draft: ApprovalRequestDraft) {
        self.set_status(format!("approval requested: {}", draft.title));
        if self
            .approval
            .pending()
            .is_some_and(|old| old.facts.same_request(&draft))
        {
            // Duplicate delivery also enters the only write point, without allocating.
            let id = self
                .approval
                .pending()
                .and_then(|p| p.entry_id())
                .expect("issued card");
            self.approval.change(ApprovalChange::Adopt(draft, id));
        } else {
            let id = self.transcript_allocate_card();
            self.approval.change(ApprovalChange::Adopt(draft, id));
        }
    }

    /// Give a new queued draft a fresh id; callers never receive the card value.
    pub(crate) fn enqueue_approval(
        &mut self,
        draft: ApprovalRequestDraft,
        queue: ApprovalQueue,
    ) -> EntryId {
        let id = self.transcript_allocate_card();
        self.approval
            .change(ApprovalChange::Enqueue(draft, id, queue));
        id
    }

    /// Focus only a still-queued id, moving the active card to the same queue.
    pub(crate) fn focus_approval(&mut self, id: EntryId, queue: ApprovalQueue) {
        self.approval.change(ApprovalChange::Focus(id, queue));
    }

    /// Retire the active card and show the backend's next queued card.
    pub(crate) fn promote_approval(&mut self, queue: ApprovalQueue) {
        self.approval.change(ApprovalChange::Promote(queue));
    }

    /// Retire an active or queued id; unknown and retired ids do nothing.
    pub(crate) fn retire_approval(&mut self, id: EntryId) {
        self.approval.change(ApprovalChange::Retire(id));
    }

    /// Discard one backend's queue without changing the active card.
    pub(crate) fn clear_approval_queue(&mut self, queue: ApprovalQueue) {
        self.approval.change(ApprovalChange::ClearQueue(queue));
    }

    /// Fence an answer by the card it answers, then copy only its immutable facts.
    pub(crate) fn approval_answer(&self, id: EntryId) -> Option<ApprovalRequestDraft> {
        self.approval
            .pending()
            .filter(|p| p.entry_id() == Some(id))
            .map(|p| p.to_draft())
    }

    /// Accept an answer command only for the id that is currently active.
    pub(crate) fn begin_approval_answer(&mut self, id: EntryId) -> bool {
        self.approval.change(ApprovalChange::BeginAnswer(id));
        self.approval
            .pending()
            .is_some_and(|p| p.entry_id() == Some(id) && p.answer_submitted)
    }

    /// Finish an answer accepted while active, even if focus moved during validation.
    pub(crate) fn submitted_approval_answer(&self, id: EntryId) -> Option<&ApprovalRequestDraft> {
        self.approval
            .live()
            .find(|p| p.entry_id() == Some(id) && p.answer_submitted)
            .map(|p| p.facts())
    }
}
