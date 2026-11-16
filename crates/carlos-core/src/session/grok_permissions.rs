//! Present offered ACP permission choices while keeping reply ids and option ids private.

use std::collections::HashSet;

use anyhow::{ensure, Context, Result};
use carlos_proto::commands::FieldAnswers;
use carlos_proto::session::{Approval, ApprovalInput, Backend, InputField, InputOption};
use serde_json::{json, Value};

use super::approval_state::{
    ApprovalChoice, ApprovalQueue, ApprovalRequestDraft, ApprovalRequestKind,
};
use super::state::SessionState;

impl SessionState {
    pub(super) fn receive_grok_permission(&mut self, method: &str, params: &Value) {
        if self.events.backend != Backend::Grok {
            return;
        }
        let Some(token) = params["token"].as_str() else {
            return;
        };
        if method == "carlos/grok/callback-retired" {
            self.retire_grok_permission(&json!(token));
            return;
        }
        if self
            .approval()
            .live()
            .any(|card| card.facts().request_id == token)
        {
            return;
        }
        let request = &params["request"];
        let title = request["params"]["toolCall"]["title"]
            .as_str()
            .unwrap_or("Grok permission request");
        let draft = ApprovalRequestDraft {
            request_id: json!(token),
            method: "session/request_permission".into(),
            kind: ApprovalRequestKind::GrokPermission,
            title: super::text::safe_label(super::task_rows::clipped(title, 4096)),
            detail_lines: vec![super::text::safe_text(super::task_rows::clipped(
                &request["params"]["toolCall"]["rawInput"].to_string(),
                16384,
            ))],
            requested_permissions: Some(request.clone()),
            can_accept_for_session: false,
            can_decline: false,
            can_cancel: true,
        };
        if self.approval().pending().is_none() {
            let _ = self.set_pending_approval(draft);
        } else {
            self.enqueue_approval(draft, ApprovalQueue::Grok);
        }
    }

    pub(super) fn retire_grok_permission(&mut self, token: &Value) {
        let _ = self.retire_queued_approvals(token, ApprovalQueue::Grok);
        if self
            .approval()
            .pending()
            .is_some_and(|card| &card.facts().request_id == token)
        {
            let _ = self.promote_approval(ApprovalQueue::Grok);
        }
    }

    pub(super) fn clear_grok_permissions(&mut self) {
        let _ = self.clear_approval_queue(ApprovalQueue::Grok);
        if self
            .approval()
            .pending()
            .is_some_and(|card| card.facts().kind == ApprovalRequestKind::GrokPermission)
        {
            let _ = self.retire_active_approval();
        }
    }
}

pub(super) fn input(request: &Value) -> ApprovalInput {
    match permission_field(request) {
        Ok(field) => ApprovalInput::Fields {
            fields: vec![field],
        },
        Err(error) => ApprovalInput::Unsupported {
            reason: error.to_string(),
        },
    }
}

fn permission_field(request: &Value) -> Result<InputField> {
    let options = offered_options(request)?;
    Ok(InputField {
        key: "permission".into(),
        title: "Choose Grok's permission".into(),
        description: "Grok controls how long a remembered choice lasts.".into(),
        secret: false,
        multiple: false,
        other: false,
        notes_only: false,
        required: true,
        schema: json!({"type":"integer"}),
        options: options
            .iter()
            .enumerate()
            .map(|(index, option)| InputOption {
                value: json!(index),
                label: super::text::safe_label(option["name"].as_str().expect("checked name")),
                description: String::new(),
            })
            .collect(),
    })
}

fn offered_options(request: &Value) -> Result<&[Value]> {
    let options = request["params"]["options"]
        .as_array()
        .context("Grok supplied no permission options")?;
    ensure!(
        !options.is_empty() && options.len() <= 256,
        "Grok supplied an unsupported number of options"
    );
    let mut ids = HashSet::new();
    for option in options {
        let id = option["optionId"]
            .as_str()
            .filter(|id| !id.is_empty() && id.len() <= 4096)
            .context("Grok supplied an invalid permission option id")?;
        ensure!(ids.insert(id), "Grok repeated a permission option id");
        ensure!(
            matches!(
                option["kind"].as_str(),
                Some("allow_once" | "allow_always" | "reject_once" | "reject_always")
            ),
            "Grok supplied an unsupported permission option kind"
        );
        ensure!(
            option["name"]
                .as_str()
                .is_some_and(|name| !name.is_empty() && name.len() <= 4096),
            "Grok supplied an invalid permission option name"
        );
    }
    Ok(options)
}

pub(super) fn answer(
    facts: &ApprovalRequestDraft,
    card: &Approval,
    value: &Value,
) -> Result<(ApprovalChoice, Value)> {
    ensure!(
        value.to_string().len() <= 65536,
        "permission answer exceeds 64 KiB"
    );
    let submitted: FieldAnswers =
        serde_json::from_value(value.clone()).context("invalid permission answer")?;
    let Some(ApprovalInput::Fields { fields }) = &card.input else {
        anyhow::bail!("permission options are unsupported");
    };
    ensure!(
        submitted.fields.len() == 1 && fields.len() == 1,
        "choose exactly one permission field"
    );
    let field = &fields[0];
    let answer = &submitted.fields[0];
    ensure!(answer.key == field.key, "unknown permission field");
    super::control_answers::validate_field(field, answer, false)?;
    let index = answer.values[0]
        .as_u64()
        .context("invalid permission choice")? as usize;
    let options = offered_options(
        facts
            .requested_permissions
            .as_ref()
            .context("native permission is missing")?,
    )?;
    let option = options
        .get(index)
        .context("permission choice is not offered")?;
    let choice = if matches!(
        option["kind"].as_str(),
        Some("reject_once" | "reject_always")
    ) {
        ApprovalChoice::Decline
    } else {
        ApprovalChoice::Accept
    };
    Ok((
        choice,
        json!({"outcome":{"outcome":"selected","optionId":option["optionId"]}}),
    ))
}

#[cfg(test)]
#[path = "grok_permission_tests.rs"]
mod tests;
