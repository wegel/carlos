//! Keep the original request facts for fixed consent lifetime cases.

use carlos_core::session::approval_state::{ApprovalRequestDraft, ApprovalRequestKind};
use serde_json::json;

pub(super) fn request(kind: ApprovalRequestKind, id: &str) -> ApprovalRequestDraft {
    carlos_core::session::approval_state::ApprovalRequestDraft {
        request_id: json!(id),
        method: "claude/native".into(),
        kind,
        title: format!("Approve {kind:?}"),
        detail_lines: vec!["native request details".into()],
        requested_permissions: Some(json!({"defaultToNo":true,"toolName":"Bash",
            "requestedSchema":{"type":"object","properties":{}}})),
        can_accept_for_session: true,
        can_decline: true,
        can_cancel: true,
    }
}
