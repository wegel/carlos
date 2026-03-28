use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ApprovalChoice {
    Accept,
    AcceptForSession,
    Decline,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ApprovalRequestKind {
    CommandExecution,
    FileChange,
    Permissions,
    LegacyExecCommand,
    LegacyApplyPatch,
}

#[derive(Debug, Clone)]
pub(super) struct PendingApprovalRequest {
    pub(super) request_id: Value,
    pub(super) method: String,
    pub(super) kind: ApprovalRequestKind,
    pub(super) title: String,
    pub(super) detail_lines: Vec<String>,
    pub(super) requested_permissions: Option<Value>,
    pub(super) can_accept_for_session: bool,
    pub(super) can_decline: bool,
    pub(super) can_cancel: bool,
}

impl PendingApprovalRequest {
    pub(super) fn response_for_choice(&self, choice: ApprovalChoice) -> Option<Value> {
        match self.kind {
            ApprovalRequestKind::CommandExecution => match choice {
                ApprovalChoice::Accept => Some(json!({ "decision": "accept" })),
                ApprovalChoice::AcceptForSession if self.can_accept_for_session => {
                    Some(json!({ "decision": "acceptForSession" }))
                }
                ApprovalChoice::Decline if self.can_decline => {
                    Some(json!({ "decision": "decline" }))
                }
                ApprovalChoice::Cancel if self.can_cancel => Some(json!({ "decision": "cancel" })),
                _ => None,
            },
            ApprovalRequestKind::FileChange => match choice {
                ApprovalChoice::Accept => Some(json!({ "decision": "accept" })),
                ApprovalChoice::AcceptForSession if self.can_accept_for_session => {
                    Some(json!({ "decision": "acceptForSession" }))
                }
                ApprovalChoice::Decline if self.can_decline => {
                    Some(json!({ "decision": "decline" }))
                }
                ApprovalChoice::Cancel if self.can_cancel => Some(json!({ "decision": "cancel" })),
                _ => None,
            },
            ApprovalRequestKind::Permissions => match choice {
                ApprovalChoice::Accept => Some(json!({
                    "permissions": self.requested_permissions.clone().unwrap_or_else(|| json!({}))
                })),
                ApprovalChoice::AcceptForSession if self.can_accept_for_session => Some(json!({
                    "permissions": self.requested_permissions.clone().unwrap_or_else(|| json!({})),
                    "scope": "session"
                })),
                ApprovalChoice::Decline if self.can_decline => Some(json!({
                    "permissions": {}
                })),
                _ => None,
            },
            ApprovalRequestKind::LegacyExecCommand | ApprovalRequestKind::LegacyApplyPatch => {
                match choice {
                    ApprovalChoice::Accept => Some(json!({ "decision": "approved" })),
                    ApprovalChoice::AcceptForSession if self.can_accept_for_session => {
                        Some(json!({ "decision": "approved_for_session" }))
                    }
                    ApprovalChoice::Decline if self.can_decline => {
                        Some(json!({ "decision": "denied" }))
                    }
                    ApprovalChoice::Cancel if self.can_cancel => {
                        Some(json!({ "decision": "abort" }))
                    }
                    _ => None,
                }
            }
        }
    }
}

pub(super) struct ApprovalState {
    pub(super) pending: Option<PendingApprovalRequest>,
}

impl ApprovalState {
    pub(super) fn new() -> Self {
        Self { pending: None }
    }
}
