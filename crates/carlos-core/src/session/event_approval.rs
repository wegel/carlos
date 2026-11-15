//! Export only client-visible approval facts, excluding native routing payloads.

use carlos_proto::events::Event;
use carlos_proto::session::{Approval, ApprovalInput, Decision, InputField, InputOption};

use super::approval_state::{
    ApprovalChoice, ApprovalRequestDraft, ApprovalRequestKind, PendingApprovalRequest,
};
use super::claude::callback_questions::ClaudeQuestion;
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
            .map(|card| {
                let mut exported = export_card(
                    card,
                    active == Some(card.entry_id()),
                    self.approval_blocks(card),
                    self.approval_browser_completed(card),
                    self.approval_caller(card),
                );
                exported.input_error = self
                    .codex_callbacks()
                    .get(&card.facts().request_id)
                    .and_then(|callback| callback.schema_error.clone());
                exported
            })
            .collect()
    }

    pub(super) fn approval_blocks(&self, card: &PendingApprovalRequest) -> bool {
        self.codex_callbacks()
            .get(&card.facts().request_id)
            .is_none_or(|callback| callback.blocking())
    }

    pub(super) fn approval_browser_completed(&self, card: &PendingApprovalRequest) -> bool {
        card.facts()
            .request_id
            .as_str()
            .is_some_and(|id| self.claude_browser_completed(id))
    }

    pub(super) fn approval_caller(&self, card: &PendingApprovalRequest) -> Option<String> {
        let facts = card.facts();
        let payload = self
            .codex_callbacks()
            .get(&facts.request_id)
            .map(|entry| entry.params())
            .or_else(|| {
                (facts.method == "claude/native")
                    .then_some(facts.requested_permissions.as_ref())
                    .flatten()
            })?;
        let label = super::questions::caller_label(&facts.method, payload);
        Some(if label.len() > 4096 {
            format!("{}…", super::task_rows::clipped(&label, 4093))
        } else {
            label
        })
    }
}

pub(super) fn export_card(
    card: &PendingApprovalRequest,
    active: bool,
    blocking: bool,
    browser_completed: bool,
    caller: Option<String>,
) -> Approval {
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
    let mut details = facts.detail_lines.clone();
    if browser_completed {
        details.insert(0, "The server reported browser completion. Your consent is still required; decline or cancel remains available.".into());
    }
    Approval {
        id: card.entry_id().to_wire(),
        kind: kind.to_owned(),
        title: facts.title.clone(),
        caller,
        display_id: Some(request_label(&facts.request_id)),
        details,
        choices,
        input,
        input_error: None,
        submitted: card.submitted(),
        active,
        blocking,
        default_to_no: facts
            .requested_permissions
            .as_ref()
            .is_some_and(|payload| payload["defaultToNo"] == true),
    }
}

fn request_label(id: &serde_json::Value) -> String {
    let (text, shortened) = match id {
        serde_json::Value::String(id) => {
            let prefix = super::task_rows::clipped(id, 4096);
            (
                serde_json::to_string(prefix).expect("native string id"),
                prefix.len() < id.len(),
            )
        }
        serde_json::Value::Number(id) => (id.to_string(), false),
        _ => ("unavailable".into(), false),
    };
    let text = super::text::safe_label(&text);
    if shortened || text.len() > 4096 {
        format!("{}…", super::task_rows::clipped(&text, 4093))
    } else {
        text
    }
}

fn export_input(card: &PendingApprovalRequest) -> Option<ApprovalInput> {
    let facts = card.facts();
    let payload = facts.requested_permissions.as_ref()?;
    match facts.kind {
        ApprovalRequestKind::CodexQuestion => Some(export_fields(
            question_fields(payload).map_err(|error| error.to_string()),
        )),
        ApprovalRequestKind::CodexForm | ApprovalRequestKind::ClaudeQuestion => Some(export_form(
            payload,
            facts.kind == ApprovalRequestKind::CodexForm,
        )),
        ApprovalRequestKind::ClaudeNative if payload["toolName"] == "ExitPlanMode" => {
            ClaudeQuestion::from_payload("permission", payload)
                .unsupported
                .map(|reason| ApprovalInput::Unsupported { reason })
        }
        ApprovalRequestKind::ClaudeNative if payload["toolName"] == "AskUserQuestion" => {
            Some(export_fields(ask_fields(&payload["input"])))
        }
        _ => None,
    }
}

fn export_form(payload: &serde_json::Value, codex: bool) -> ApprovalInput {
    let unsupported = |reason: String| ApprovalInput::Unsupported { reason };
    if payload["mode"] == "url" {
        let url = payload["url"].as_str().unwrap_or("");
        return match crate::native_form::url_host(url) {
            Ok(_) => ApprovalInput::Browser {
                url: url.to_owned(),
            },
            Err(error) => unsupported(error.to_string()),
        };
    }
    if let Some(reason) = payload["formIssue"].as_str() {
        return unsupported(reason.to_owned());
    }
    if codex && payload["mode"] != "form" {
        return unsupported(format!(
            "Unsupported request mode: {}",
            payload["mode"].as_str().unwrap_or("missing")
        ));
    }
    match payload.get("requestedSchema") {
        Some(schema) => ApprovalInput::Form {
            schema: schema.clone(),
        },
        None => unsupported("The original form has no schema".into()),
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
                    other: field.other() || field.options().is_empty(),
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
        if !offered
            || matches!(decision, Decision::Once | Decision::Session)
                && matches!(input, Some(ApprovalInput::Unsupported { .. }))
        {
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
            || choice == ApprovalChoice::Cancel
                && facts.kind == ApprovalRequestKind::CodexQuestion
                && facts
                    .requested_permissions
                    .as_ref()
                    .and_then(super::control_answers::question_interrupt_params)
                    .is_some()
            || facts.response_for_choice(choice).is_some();
        supported.then_some(decision)
    })
    .collect()
}

#[cfg(test)]
#[path = "event_approval_labels_tests.rs"]
mod tests;
