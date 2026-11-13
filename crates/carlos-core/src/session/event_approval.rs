//! Export only client-visible approval facts, excluding native routing payloads.

use carlos_proto::session::{Approval, ApprovalInput, Decision};

use super::approval_state::{ApprovalChoice, ApprovalRequestKind, PendingApprovalRequest};
use super::state::SessionState;

impl SessionState {
    pub(super) fn export_approvals(&self) -> Vec<Approval> {
        let active = self
            .approval
            .pending()
            .map(PendingApprovalRequest::entry_id);
        self.approval
            .live()
            .map(|card| export_card(card, active == Some(card.entry_id())))
            .collect()
    }
}

fn export_card(card: &PendingApprovalRequest, active: bool) -> Approval {
    let facts = card.facts();
    let choices = [
        (ApprovalChoice::Accept, Decision::Once),
        (ApprovalChoice::AcceptForSession, Decision::Session),
        (ApprovalChoice::Decline, Decision::Deny),
        (ApprovalChoice::Cancel, Decision::Cancel),
    ]
    .into_iter()
    .filter_map(|(choice, decision)| {
        let offered = match decision {
            Decision::Once => facts.can_accept_once(),
            Decision::Session => facts.can_accept_for_session,
            Decision::Deny => facts.can_decline,
            Decision::Cancel => facts.can_cancel,
        };
        offered
            .then(|| facts.response_for_choice(choice))
            .flatten()
            .map(|_| decision)
    })
    .collect();
    let kind = match facts.kind {
        ApprovalRequestKind::CommandExecution | ApprovalRequestKind::LegacyExecCommand => "command",
        ApprovalRequestKind::FileChange | ApprovalRequestKind::LegacyApplyPatch => "file_change",
        ApprovalRequestKind::Permissions => "permissions",
        ApprovalRequestKind::ClaudeExitPlanMode => "plan",
        ApprovalRequestKind::ClaudeNative => "tool",
        ApprovalRequestKind::ClaudeQuestion | ApprovalRequestKind::CodexQuestion => "question",
        ApprovalRequestKind::CodexForm => "form",
    };
    Approval {
        id: card.entry_id().to_wire(),
        kind: kind.to_owned(),
        title: facts.title.clone(),
        details: facts.detail_lines.clone(),
        choices,
        input: export_input(card),
        submitted: card.submitted(),
        active,
    }
}

fn export_input(card: &PendingApprovalRequest) -> Option<ApprovalInput> {
    let facts = card.facts();
    let payload = facts.requested_permissions.as_ref()?;
    match facts.kind {
        ApprovalRequestKind::CodexQuestion => Some(ApprovalInput::Questions {
            questions: payload.get("questions")?.clone(),
        }),
        ApprovalRequestKind::CodexForm | ApprovalRequestKind::ClaudeQuestion => {
            if payload["mode"] == "url" {
                Some(ApprovalInput::Browser {
                    url: payload.get("url")?.as_str()?.to_owned(),
                })
            } else {
                Some(ApprovalInput::Form {
                    schema: payload.get("requestedSchema")?.clone(),
                })
            }
        }
        ApprovalRequestKind::ClaudeNative if payload["toolName"] == "AskUserQuestion" => {
            Some(ApprovalInput::Questions {
                questions: payload.get("input")?.get("questions")?.clone(),
            })
        }
        _ => None,
    }
}
