//! Return only synchronous row and client-control effects from a shared backend message.

use super::actions::ServerRequestAction;
use super::approval_reply::ApprovalCleared;
use super::entry_id::EntryId;
use super::item_changes::ItemChanges;
use super::thread_commands::ThreadChanged;

/// Preserve the extra-hint reset for a duplicate card, including an absent current card.
pub(crate) struct ApprovalChanged {
    pub(crate) clear_extra_lines: bool,
    pub(crate) current: Option<EntryId>,
    pub(crate) retired: Vec<EntryId>,
}

/// Carry one shared message's changes for immediate consumption by the client.
#[derive(Default)]
pub(crate) struct CommonUpdate {
    pub(crate) items: ItemChanges,
    pub(crate) thread: Option<ThreadChanged>,
    pub(crate) approval: Option<ApprovalChanged>,
    pub(crate) cleared: Option<ApprovalCleared>,
    pub(crate) clear_quota_countdown: bool,
    pub(crate) action: Option<ServerRequestAction>,
}
