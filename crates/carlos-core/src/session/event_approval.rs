//! Export only client-visible approval facts, excluding native routing payloads.

use carlos_proto::events::Event;
use carlos_proto::session::{Approval, ApprovalInput, Decision, InputField, InputOption};

use super::approval_state::{
    ApprovalChoice, ApprovalRequestDraft, ApprovalRequestKind, PendingApprovalRequest,
};
use super::entry_id::EntryId;
use super::events::Published;
use super::questions::{ask_fields, question_fields, QuestionField};
use super::state::SessionState;
use super::transcript_journal::MAX_PENDING;

impl SessionState {
    pub(super) fn record_approval_decision(&mut self, card: EntryId, choice: ApprovalChoice) {
        let decision = match choice {
            ApprovalChoice::Accept => Decision::Once,
            ApprovalChoice::AcceptForSession => Decision::Session,
            ApprovalChoice::Decline => Decision::Deny,
            ApprovalChoice::Cancel => Decision::Cancel,
        };
        if self.events.decisions.len() < MAX_PENDING {
            self.events.decisions.push((card, decision));
        } else {
            self.events.decision_overflow = true;
        }
    }

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
    let input = export_input(card);
    let choices = export_choices(facts, input.as_ref());
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
        input,
        submitted: card.submitted(),
        active,
        default_to_no: facts
            .requested_permissions
            .as_ref()
            .is_some_and(|payload| payload["defaultToNo"] == true),
    }
}

fn export_input(card: &PendingApprovalRequest) -> Option<ApprovalInput> {
    let facts = card.facts();
    let payload = facts.requested_permissions.as_ref()?;
    match facts.kind {
        ApprovalRequestKind::CodexQuestion => Some(export_fields(
            question_fields(payload).map_err(|error| error.to_string()),
        )),
        ApprovalRequestKind::CodexForm | ApprovalRequestKind::ClaudeQuestion => {
            if payload["mode"] == "url" {
                Some(ApprovalInput::Browser {
                    url: payload.get("url")?.as_str()?.to_owned(),
                })
            } else if let Some(reason) = payload["formIssue"].as_str() {
                Some(ApprovalInput::Unsupported {
                    reason: reason.to_owned(),
                })
            } else {
                Some(ApprovalInput::Form {
                    schema: payload.get("requestedSchema")?.clone(),
                })
            }
        }
        ApprovalRequestKind::ClaudeNative if payload["toolName"] == "AskUserQuestion" => {
            Some(export_fields(ask_fields(&payload["input"])))
        }
        _ => None,
    }
}

fn export_fields(fields: Result<Vec<QuestionField>, String>) -> ApprovalInput {
    match fields {
        Ok(fields) => ApprovalInput::Fields {
            fields: fields
                .into_iter()
                .map(|field| InputField {
                    key: field.name().to_owned(),
                    title: field.title().to_owned(),
                    description: field.description().to_owned(),
                    secret: field.secret(),
                    multiple: field.multiple(),
                    other: field.other(),
                    notes_only: field.notes_only(),
                    required: field.required(),
                    schema: field.schema().clone(),
                    options: field
                        .options()
                        .iter()
                        .map(|option| InputOption {
                            value: option.value().clone(),
                            label: option.label().to_owned(),
                            description: option.description().to_owned(),
                        })
                        .collect(),
                })
                .collect(),
        },
        Err(reason) => ApprovalInput::Unsupported { reason },
    }
}

impl Published<'_> {
    pub(super) fn approval_event(&self, id: EntryId) -> Option<Event> {
        let cards = &self.session.events.approvals;
        let wire_id = id.to_wire();
        let index = cards.iter().position(|card| card.id == wire_id)?;
        Some(Event::ApprovalRequested {
            approval: cards[index].clone(),
            before: cards.get(index + 1).map(|card| card.id.clone()),
        })
    }
}

fn export_choices(facts: &ApprovalRequestDraft, input: Option<&ApprovalInput>) -> Vec<Decision> {
    [
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
        if !offered {
            return None;
        }
        // Browser consent needs no form body. Permission replies may clone a large
        // payload, so inspect their capability instead of constructing a throwaway reply.
        let supported = choice == ApprovalChoice::Accept
            && matches!(input, Some(ApprovalInput::Browser { .. }))
            || facts.kind == ApprovalRequestKind::Permissions
                && matches!(
                    choice,
                    ApprovalChoice::Accept
                        | ApprovalChoice::AcceptForSession
                        | ApprovalChoice::Decline
                )
            || facts.response_for_choice(choice).is_some();
        supported.then_some(decision)
    })
    .collect()
}
