//! Approval request types and response generation for tool-use confirmations.

use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[path = "approval_commands.rs"]
mod commands;

pub use commands::{ApprovalQueue, ApprovalState};

// --- Approval Types ---

/// User's choice when responding to an approval prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalChoice {
    Accept,
    AcceptForSession,
    Decline,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Approvalrequestkind.
pub enum ApprovalRequestKind {
    CommandExecution,
    FileChange,
    Permissions,
    LegacyExecCommand,
    LegacyApplyPatch,
    ClaudeExitPlanMode,
    ClaudeNative,
    ClaudeQuestion,
    CodexQuestion,
    CodexForm,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Backend request facts before the session assigns a card id.
pub struct ApprovalRequestDraft {
    pub request_id: Value,
    pub method: String,
    pub kind: ApprovalRequestKind,
    pub title: String,
    pub detail_lines: Vec<String>,
    pub requested_permissions: Option<Value>,
    pub can_accept_for_session: bool,
    pub can_decline: bool,
    pub can_cancel: bool,
}

/// A session-issued card whose id and request facts callers can only read.
#[derive(Debug)]
pub struct PendingApprovalRequest {
    entry_id: crate::session::entry_id::EntryId,
    facts: ApprovalRequestDraft,
    answer_submitted: bool,
    live: Arc<AtomicBool>,
}

/// One claimed live card; dropping the session card invalidates undispatched worker work.
pub(crate) struct ApprovalClaim {
    pub(crate) card: crate::session::entry_id::EntryId,
    pub(crate) facts: ApprovalRequestDraft,
    pub(crate) live: Arc<AtomicBool>,
}

impl Drop for PendingApprovalRequest {
    fn drop(&mut self) {
        self.live.store(false, Ordering::SeqCst);
    }
}

impl PendingApprovalRequest {
    /// Borrow the immutable backend facts of this card.
    pub fn facts(&self) -> &ApprovalRequestDraft {
        &self.facts
    }

    /// Read the card id issued by the session.
    pub fn entry_id(&self) -> crate::session::entry_id::EntryId {
        self.entry_id
    }

    /// Check whether the user already submitted this card.
    pub fn submitted(&self) -> bool {
        self.answer_submitted
    }

    /// Copy facts into a new draft without copying the card's identity.
    pub fn to_draft(&self) -> ApprovalRequestDraft {
        self.facts.clone()
    }
}

// --- Response Mapping ---

impl ApprovalRequestDraft {
    /// Distinguish a repeated delivery from a new request reusing a native id.
    pub(crate) fn same_request(&self, other: &Self) -> bool {
        // Request facts: request_id, method, kind, title, detail_lines,
        // requested_permissions (the entire original payload), can_accept_for_session,
        // can_decline, and can_cancel. These select the backend answer and everything
        // the user sees about the request; a card id never substitutes for these facts.
        self == other
    }

    /// Response for choice.
    pub fn response_for_choice(&self, choice: ApprovalChoice) -> Option<Value> {
        if choice == ApprovalChoice::Accept && !self.can_accept_once() {
            return None;
        }
        match self.kind {
            ApprovalRequestKind::CodexQuestion => match choice {
                ApprovalChoice::Decline => Some(json!({"answers":{}})),
                _ => None,
            },
            ApprovalRequestKind::CodexForm => match choice {
                ApprovalChoice::Decline => Some(json!({"action":"decline"})),
                ApprovalChoice::Cancel => Some(json!({"action":"cancel"})),
                _ => None,
            },
            ApprovalRequestKind::ClaudeNative => match choice {
                ApprovalChoice::Accept
                    if self
                        .requested_permissions
                        .as_ref()
                        .is_some_and(|payload| payload["toolName"] == "AskUserQuestion") =>
                {
                    None
                }
                ApprovalChoice::Accept => Some(json!({ "behavior": "allow" })),
                ApprovalChoice::Decline => {
                    Some(json!({ "behavior": "deny", "message": "User declined" }))
                }
                ApprovalChoice::Cancel => Some(
                    json!({ "behavior": "deny", "message": "User cancelled", "interrupt": true }),
                ),
                _ => None,
            },
            ApprovalRequestKind::ClaudeQuestion => match choice {
                ApprovalChoice::Accept => self.empty_question_answer(),
                ApprovalChoice::Decline => Some(json!({ "action": "decline" })),
                ApprovalChoice::Cancel => Some(json!({ "action": "cancel" })),
                _ => None,
            },
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

    /// Present a one-time allow only when the native server actually offers it.
    pub fn can_accept_once(&self) -> bool {
        self.requested_permissions
            .as_ref()
            .and_then(|payload| payload["availableDecisions"].as_array())
            .is_none_or(|decisions| decisions.iter().any(|decision| decision == "accept"))
    }

    fn empty_question_answer(&self) -> Option<Value> {
        let payload = self.requested_permissions.as_ref()?;
        if payload["mode"] == "url" {
            return None;
        }
        let schema = &payload["requestedSchema"];
        if schema["required"]
            .as_array()
            .is_some_and(|fields| !fields.is_empty())
            || schema["properties"]
                .as_object()
                .is_some_and(|fields| !fields.is_empty())
        {
            return None;
        }
        Some(json!({ "action": "accept", "content": {} }))
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

    /// Persists after turn completed.
    pub(crate) fn persists_after_turn_completed(&self) -> bool {
        matches!(
            self.kind,
            ApprovalRequestKind::ClaudeExitPlanMode
                | ApprovalRequestKind::ClaudeNative
                | ApprovalRequestKind::ClaudeQuestion
        )
    }
}
