//! Keep every issued approval card inside one private owner and one write point.

use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use serde_json::Value;

use super::{ApprovalRequestDraft, PendingApprovalRequest};
use crate::session::entry_id::EntryId;
use crate::session::state::SessionState;

/// Select the backend's independently ordered callback queue.
#[derive(Debug, Clone, Copy)]
pub enum ApprovalQueue {
    Claude,
    Codex,
    Grok,
}

/// Own every live card; removing a card retires its id forever.
#[derive(Debug, Default)]
pub struct ApprovalState {
    pub(in crate::session) revision: u64,
    pending: Option<PendingApprovalRequest>,
    claude: VecDeque<PendingApprovalRequest>,
    codex: VecDeque<PendingApprovalRequest>,
    grok: VecDeque<PendingApprovalRequest>,
}

impl ApprovalState {
    /// Borrow the active card without granting a setter or a copy of its identity.
    pub fn pending(&self) -> Option<&PendingApprovalRequest> {
        self.pending.as_ref()
    }

    /// Borrow the cards waiting for one backend.
    pub fn queued(&self, queue: ApprovalQueue) -> &VecDeque<PendingApprovalRequest> {
        match queue {
            ApprovalQueue::Claude => &self.claude,
            ApprovalQueue::Codex => &self.codex,
            ApprovalQueue::Grok => &self.grok,
        }
    }

    /// Visit each live card once, in active, Claude, then Codex order.
    pub fn live(&self) -> impl Iterator<Item = &PendingApprovalRequest> {
        self.pending
            .iter()
            .chain(self.claude.iter())
            .chain(self.codex.iter())
            .chain(self.grok.iter())
    }
}

// Only this module can name a command or write the card containers.
enum ApprovalChange<'a> {
    MetadataRefresh,
    Adopt(ApprovalRequestDraft, EntryId),
    Enqueue(ApprovalRequestDraft, EntryId, ApprovalQueue),
    Focus(EntryId, ApprovalQueue),
    Promote(ApprovalQueue),
    Retire(EntryId),
    RetireQueued(&'a Value, ApprovalQueue),
    ClearQueue(ApprovalQueue),
    BeginAnswer(EntryId),
    ClaimLive(EntryId),
    Reopen(EntryId, &'a Arc<AtomicBool>),
}

impl ApprovalState {
    /// Refresh readonly card details after native progress without changing any card's identity.
    pub(in crate::session) fn refresh_metadata(&mut self) {
        self.change(ApprovalChange::MetadataRefresh);
    }
    // This is the sole write point for the active card and both queues. Id-only
    // moves search the live containers. No retired-id ledger is needed: absent
    // ids cannot be inserted, and only the session allocator can mint a card.
    fn change(&mut self, change: ApprovalChange<'_>) -> Vec<EntryId> {
        self.revision = self
            .revision
            .checked_add(1)
            .expect("approval revision exhausted");
        let mut retired = Vec::new();
        match change {
            ApprovalChange::MetadataRefresh => {}
            ApprovalChange::Adopt(facts, entry_id) => {
                retired.extend(self.pending.take().map(|p| p.entry_id()));
                self.pending = Some(PendingApprovalRequest {
                    entry_id,
                    facts: Arc::new(facts),
                    answer_submitted: false,
                    live: Arc::new(AtomicBool::new(true)),
                });
            }
            ApprovalChange::Enqueue(facts, entry_id, queue) => {
                self.queue_mut(queue).push_back(PendingApprovalRequest {
                    entry_id,
                    facts: Arc::new(facts),
                    answer_submitted: false,
                    live: Arc::new(AtomicBool::new(true)),
                });
            }
            ApprovalChange::Focus(id, queue) => {
                if let Some(index) = self.queued(queue).iter().position(|p| p.entry_id() == id) {
                    let card = self.queue_mut(queue).remove(index);
                    if let Some(old) = self.pending.take() {
                        self.queue_mut(queue).push_back(old);
                    }
                    self.pending = card;
                }
            }
            ApprovalChange::Promote(queue) => {
                retired.extend(self.pending.take().map(|p| p.entry_id()));
                self.pending = self.queue_mut(queue).pop_front();
            }
            ApprovalChange::Retire(id) => {
                retired.extend(self.retire(id));
            }
            ApprovalChange::RetireQueued(id, queue) => return self.retire_queued(id, queue),
            ApprovalChange::ClearQueue(queue) => {
                retired.extend(self.queue_mut(queue).drain(..).map(|p| p.entry_id()));
            }
            ApprovalChange::ClaimLive(id) => self.claim(id),
            ApprovalChange::Reopen(id, live) => self.reopen(id, live),
            ApprovalChange::BeginAnswer(id) => {
                if let Some(card) = self.pending.as_mut().filter(|p| p.entry_id() == id) {
                    card.answer_submitted = true;
                }
            }
        }
        retired
    }

    fn live_mut(&mut self) -> impl Iterator<Item = &mut PendingApprovalRequest> {
        self.pending
            .iter_mut()
            .chain(self.claude.iter_mut())
            .chain(self.codex.iter_mut())
            .chain(self.grok.iter_mut())
    }

    fn claim(&mut self, id: EntryId) {
        if let Some(card) = self
            .live_mut()
            .find(|card| card.entry_id == id && !card.answer_submitted)
        {
            card.answer_submitted = true;
        }
    }

    fn reopen(&mut self, id: EntryId, live: &Arc<AtomicBool>) {
        if let Some(card) = self
            .live_mut()
            .find(|card| card.entry_id == id && Arc::ptr_eq(&card.live, live))
        {
            card.live.store(false, Ordering::SeqCst);
            card.live = Arc::new(AtomicBool::new(true));
            card.answer_submitted = false;
        }
    }

    fn retire(&mut self, id: EntryId) -> Option<EntryId> {
        if self.pending().is_some_and(|p| p.entry_id() == id) {
            return self.pending.take().map(|p| p.entry_id());
        }
        for queue in [&mut self.claude, &mut self.codex, &mut self.grok] {
            if let Some(index) = queue.iter().position(|p| p.entry_id() == id) {
                return queue.remove(index).map(|p| p.entry_id());
            }
        }
        None
    }

    fn retire_queued(&mut self, id: &Value, queue: ApprovalQueue) -> Vec<EntryId> {
        let mut retired = Vec::new();
        self.queue_mut(queue).retain(|p| {
            if &p.facts().request_id == id {
                retired.push(p.entry_id());
                false
            } else {
                true
            }
        });
        retired
    }

    fn queue_mut(&mut self, queue: ApprovalQueue) -> &mut VecDeque<PendingApprovalRequest> {
        match queue {
            ApprovalQueue::Claude => &mut self.claude,
            ApprovalQueue::Codex => &mut self.codex,
            ApprovalQueue::Grok => &mut self.grok,
        }
    }
}

impl SessionState {
    /// Adopt new facts, preserving an identical active card's consent key.
    #[must_use = "pass the retired ids to the view so it drops their hints"]
    pub fn set_pending_approval(&mut self, draft: ApprovalRequestDraft) -> Vec<EntryId> {
        self.set_status(format!("approval requested: {}", draft.title));
        if self
            .approval
            .pending()
            .is_some_and(|old| old.facts.same_request(&draft))
        {
            return Vec::new();
        }
        let id = self.transcript_allocate_card();
        self.approval.change(ApprovalChange::Adopt(draft, id))
    }

    /// Give a new queued draft a fresh id; callers never receive the card value.
    pub fn enqueue_approval(
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
    pub fn focus_approval(&mut self, id: EntryId, queue: ApprovalQueue) {
        self.approval.change(ApprovalChange::Focus(id, queue));
    }

    /// Retire the active card and show the backend's next queued card.
    #[must_use = "pass the retired ids to the view so it drops their hints"]
    pub fn promote_approval(&mut self, queue: ApprovalQueue) -> Vec<EntryId> {
        self.approval.change(ApprovalChange::Promote(queue))
    }

    /// Retire an active or queued id; unknown and retired ids do nothing.
    #[must_use = "pass the retired ids to the view so it drops their hints"]
    pub fn retire_approval(&mut self, id: EntryId) -> Vec<EntryId> {
        self.approval.change(ApprovalChange::Retire(id))
    }

    /// Retire queued cards matching the backend's native request id in one pass.
    #[must_use = "pass the retired ids to the view so it drops their hints"]
    pub(crate) fn retire_queued_approvals(
        &mut self,
        id: &Value,
        queue: ApprovalQueue,
    ) -> Vec<EntryId> {
        self.approval
            .change(ApprovalChange::RetireQueued(id, queue))
    }

    /// Retire the active card without promoting a queued request.
    #[must_use = "pass the retired ids to the view so it drops their hints"]
    pub(crate) fn retire_active_approval(&mut self) -> Vec<EntryId> {
        self.approval
            .pending()
            .map(|p| p.entry_id())
            .map(|id| self.retire_approval(id))
            .unwrap_or_default()
    }

    /// Discard one backend's queue without changing the active card.
    #[must_use = "pass the retired ids to the view so it drops their hints"]
    pub fn clear_approval_queue(&mut self, queue: ApprovalQueue) -> Vec<EntryId> {
        self.approval.change(ApprovalChange::ClearQueue(queue))
    }

    /// Fence an answer by the card it answers, then copy only its immutable facts.
    pub fn approval_answer(&self, id: EntryId) -> Option<ApprovalRequestDraft> {
        self.approval
            .pending()
            .filter(|p| p.entry_id() == id)
            .map(|p| p.to_draft())
    }

    /// Accept an answer command only for the id that is currently active.
    pub fn begin_approval_answer(&mut self, id: EntryId) -> bool {
        self.approval.change(ApprovalChange::BeginAnswer(id));
        self.approval
            .pending()
            .is_some_and(|p| p.entry_id() == id && p.answer_submitted)
    }

    /// Claim any exact live card once without changing another client's focus.
    pub(crate) fn claim_live_approval(&mut self, id: EntryId) -> Option<super::ApprovalClaim> {
        let card = self
            .approval
            .live()
            .find(|card| card.entry_id == id && !card.answer_submitted)?;
        let claim = super::ApprovalClaim {
            card: id,
            facts: card.facts.clone(),
            live: card.live.clone(),
        };
        self.approval.change(ApprovalChange::ClaimLive(id));
        Some(claim)
    }

    /// Reopen only a claimed card whose worker failed before writing a native answer.
    pub(crate) fn reopen_claimed_approval(&mut self, claim: &super::ApprovalClaim) {
        self.approval
            .change(ApprovalChange::Reopen(claim.card, &claim.live));
    }

    /// Finish an answer accepted while active, even if focus moved during validation.
    pub fn submitted_approval_answer(&self, id: EntryId) -> Option<&ApprovalRequestDraft> {
        self.approval
            .live()
            .find(|p| p.entry_id() == id && p.answer_submitted)
            .map(|p| p.facts())
    }
}
