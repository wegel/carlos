//! Validate neutral approval answers before constructing a backend-specific callback reply.

use std::collections::HashSet;

use anyhow::{bail, ensure, Context, Result};
use carlos_proto::commands::{ApprovalAnswer, FieldAnswer, FieldAnswers};
use carlos_proto::session::{Approval, ApprovalInput, Decision, InputField};
use serde_json::{json, Map, Value};

use super::approval_state::{ApprovalChoice, ApprovalRequestDraft, ApprovalRequestKind};

pub(super) fn convert(
    facts: &ApprovalRequestDraft,
    card: &Approval,
    answer: &ApprovalAnswer,
) -> Result<(ApprovalChoice, Value)> {
    match answer {
        ApprovalAnswer::Decision { decision } => {
            ensure!(
                card.choices.contains(decision),
                "choice is not offered by this card"
            );
            let choice = choice(*decision);
            let body = if *decision == Decision::Once
                && matches!(card.input, Some(ApprovalInput::Browser { .. }))
            {
                if let Some(ApprovalInput::Browser { url }) = &card.input {
                    crate::native_form::url_host(url)?;
                }
                json!({"action":"accept"})
            } else {
                facts
                    .response_for_choice(choice)
                    .context("backend does not support this choice")?
            };
            Ok((choice, body))
        }
        ApprovalAnswer::Input { value } => {
            ensure!(
                facts.can_accept_once(),
                "card does not offer accepting an answer"
            );
            let body = match card
                .input
                .as_ref()
                .context("card does not accept structured input")?
            {
                ApprovalInput::Fields { fields } => field_reply(facts, fields, value)?,
                ApprovalInput::Form { schema } => {
                    crate::native_form::validate(schema, value)?;
                    json!({"action":"accept","content":value})
                }
                _ => bail!("card does not accept this structured answer"),
            };
            Ok((ApprovalChoice::Accept, body))
        }
    }
}

fn choice(decision: Decision) -> ApprovalChoice {
    match decision {
        Decision::Once => ApprovalChoice::Accept,
        Decision::Session => ApprovalChoice::AcceptForSession,
        Decision::Deny => ApprovalChoice::Decline,
        Decision::Cancel => ApprovalChoice::Cancel,
    }
}

fn field_reply(
    facts: &ApprovalRequestDraft,
    fields: &[InputField],
    value: &Value,
) -> Result<Value> {
    ensure!(
        value.to_string().len() <= 65536,
        "field answer exceeds 64 KiB"
    );
    let submitted: FieldAnswers =
        serde_json::from_value(value.clone()).context("invalid field answer")?;
    let mut seen = HashSet::new();
    let mut answers = Map::new();
    for answer in submitted.fields {
        ensure!(
            seen.insert(answer.key.clone()),
            "answer repeats a field key"
        );
        let field = fields
            .iter()
            .find(|field| field.key == answer.key)
            .context("unknown field key")?;
        let values = validate_field(field, &answer)?;
        let value = if facts.kind == ApprovalRequestKind::CodexQuestion {
            json!({"answers":values})
        } else {
            json!(values.join(", "))
        };
        answers.insert(answer.key, value);
    }
    ensure!(
        fields
            .iter()
            .all(|field| !field.required || seen.contains(&field.key)),
        "required field is missing"
    );
    if facts.kind == ApprovalRequestKind::CodexQuestion {
        return Ok(json!({"answers":answers}));
    }
    ensure!(
        facts.kind == ApprovalRequestKind::ClaudeNative,
        "backend does not support these fields"
    );
    let mut input = facts
        .requested_permissions
        .as_ref()
        .context("native question payload is missing")?["input"]
        .clone();
    input["answers"] = Value::Object(answers);
    Ok(json!({"behavior":"allow","updatedInput":input}))
}

fn validate_field(field: &InputField, answer: &FieldAnswer) -> Result<Vec<String>> {
    ensure!(
        field.multiple || answer.values.len() <= 1,
        "field accepts only one value"
    );
    let mut values = Vec::new();
    for (index, value) in answer.values.iter().enumerate() {
        ensure!(
            !answer.values[..index].contains(value),
            "field repeats a value"
        );
        let option = field
            .options
            .iter()
            .find(|option| &option.value == value)
            .context("value is not offered")?;
        values.push(option.label.clone());
    }
    let text = answer.text.as_deref().unwrap_or("");
    if !text.is_empty() {
        ensure!(field.other, "field does not offer free text");
        ensure!(
            !field.notes_only || !values.is_empty(),
            "notes require a listed value"
        );
        values.push(text.to_owned());
    }
    ensure!(
        !field.required || !values.is_empty(),
        "required field is empty"
    );
    Ok(values)
}
