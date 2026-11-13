//! Return only synchronous row and client-control effects from a shared backend message.

use super::actions::ServerRequestAction;
use super::approval_reply::ApprovalCleared;
use super::entry_id::EntryId;
use super::item_changes::ItemChanges;
use super::thread_commands::ThreadChanged;

/// Preserve the extra-hint reset for a duplicate card, including an absent current card.
pub struct ApprovalChanged {
    pub clear_extra_lines: bool,
    pub current: Option<EntryId>,
    pub retired: Vec<EntryId>,
}

/// Carry one shared message's changes for immediate consumption by the client.
#[derive(Default)]
pub struct CommonUpdate {
    pub items: ItemChanges,
    pub thread: Option<ThreadChanged>,
    pub approval: Option<ApprovalChanged>,
    pub cleared: Option<ApprovalCleared>,
    pub clear_quota_countdown: bool,
    pub action: Option<ServerRequestAction>,
}
