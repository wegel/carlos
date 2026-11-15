//! Describe the existing backend replies and controls requested by one accepted message.

use serde_json::Value;

/// Return one existing backend reply or control for immediate execution by the caller.
pub enum ServerRequestAction {
    ApplyRalphToggle,
    CodexQueuePage {
        token: u64,
        cursor: Value,
    },
    ValidateCodexForm {
        id: Value,
        schema: Value,
        answer: Option<Value>,
        revision: u64,
    },
    ReplyNative {
        card_id: crate::session::entry_id::EntryId,
        result: Value,
    },
    ClaudeControl {
        method: String,
        params: Value,
    },
    ApplyClaudeSettings,
    RefreshContext,
    ReplyError {
        request_id: Value,
        code: i64,
        message: String,
    },
}
