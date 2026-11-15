//! Report Claude session facts and correlated submitted text without reading a client editor.

use carlos_proto::submissions::{ClientOperation, SubmissionOutcome};
use serde_json::Value;

use super::callbacks::ClaudeCallback;
use crate::session::actions::ServerRequestAction;
use crate::session::approval_reply::ApprovalCleared;
use crate::session::codex::callback_commands::CallbackRetired;
use crate::session::entry_id::EntryId;
use crate::session::item_changes::ItemChanges;
use crate::session::thread_commands::ThreadChanged;

/// Report only local changes and the existing next backend action.
#[derive(Default)]
pub struct ClaudeUpdate {
    pub items: ItemChanges,
    pub effects: Vec<ClaudeEffect>,
    pub action: Option<ServerRequestAction>,
    pub(in crate::session) history_frame: Option<Value>,
}

/// Session changes; clients choose their own focus, draft recovery and scroll behavior.
pub enum ClaudeEffect {
    TaskStop(crate::session::task_stop_reply::TaskStopReply),
    Callback(ClaudeCallback),
    Cancelled {
        id: Option<String>,
        card: Option<EntryId>,
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
    EditAdopted(ClientOperation),
    Submission(SubmissionOutcome),
    SessionClosed,
}
