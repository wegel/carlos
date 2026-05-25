//! Approval request types and response generation for tool-use confirmations.

use serde_json::{json, Value};

// --- Approval Types ---

/// User's choice when responding to an approval prompt.
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
    ClaudeExitPlanMode,
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

// --- Response Mapping ---

impl PendingApprovalRequest {
    pub(super) fn response_for_choice(&self, choice: ApprovalChoice) -> Option<Value> {
        match self.kind {
            ApprovalRequestKind::CommandExecution | ApprovalRequestKind::FileChange => {
                self.decision_response(choice, "accept", "acceptForSession", "decline", "cancel")
            }
            ApprovalRequestKind::LegacyExecCommand | ApprovalRequestKind::LegacyApplyPatch => self
                .decision_response(
                    choice,
                    "approved",
                    "approved_for_session",
                    "denied",
                    "abort",
                ),
            ApprovalRequestKind::ClaudeExitPlanMode => match choice {
                ApprovalChoice::Accept => Some(json!({ "decision": "accept" })),
                ApprovalChoice::Decline if self.can_decline => {
                    Some(json!({ "decision": "decline" }))
                }
                ApprovalChoice::Cancel if self.can_cancel => Some(json!({ "decision": "cancel" })),
                _ => None,
            },
            ApprovalRequestKind::Permissions => self.permissions_response(choice),
        }
    }

    fn decision_response(
        &self,
        choice: ApprovalChoice,
        accept: &str,
        accept_for_session: &str,
        decline: &str,
        cancel: &str,
    ) -> Option<Value> {
        match choice {
            ApprovalChoice::Accept => Some(json!({ "decision": accept })),
            ApprovalChoice::AcceptForSession if self.can_accept_for_session => {
                Some(json!({ "decision": accept_for_session }))
            }
            ApprovalChoice::Decline if self.can_decline => Some(json!({ "decision": decline })),
            ApprovalChoice::Cancel if self.can_cancel => Some(json!({ "decision": cancel })),
            _ => None,
        }
    }

    fn permissions_response(&self, choice: ApprovalChoice) -> Option<Value> {
        match choice {
            ApprovalChoice::Accept => Some(json!({
                "permissions": self.requested_permissions.clone().unwrap_or_else(|| json!({}))
            })),
            ApprovalChoice::AcceptForSession if self.can_accept_for_session => Some(json!({
                "permissions": self.requested_permissions.clone().unwrap_or_else(|| json!({})),
                "scope": "session"
            })),
            ApprovalChoice::Decline if self.can_decline => Some(json!({ "permissions": {} })),
            _ => None,
        }
    }

    pub(super) fn persists_after_turn_completed(&self) -> bool {
        matches!(self.kind, ApprovalRequestKind::ClaudeExitPlanMode)
    }
}

// --- Approval State ---

pub(super) struct ApprovalState {
    pub(super) pending: Option<PendingApprovalRequest>,
}

impl ApprovalState {
    pub(super) fn new() -> Self {
        Self { pending: None }
    }
}
