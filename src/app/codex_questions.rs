//! Build Codex question editors and native replies without changing prompt text.

use std::collections::HashSet;

use anyhow::{ensure, Context, Result};
use serde_json::{json, Map, Value};

use super::claude_question_field::{AnswerField, AnswerOption};
use super::claude_question_state::{mcp_fields, safe_label, safe_text, NativeDraft};
use super::state::{ApprovalRequestKind, PendingApprovalRequest};

/// Build a bounded native question/form or an explicit unsupported-mode editor.
pub(super) fn build(
    method: &str,
    params: &Value,
    id: Value,
) -> Result<(PendingApprovalRequest, NativeDraft)> {
    let mut draft = NativeDraft::from_payload("", &Value::Null);
    let mut details = vec![format!(
        "Thread: {}",
        safe_label(
            params["threadId"]
                .as_str()
                .context("Callback has no thread ID")?
        )
    )];
    if let Some(turn) = params["turnId"].as_str() {
        details.push(format!("Turn: {}", safe_label(turn)));
    }
    let (kind, title) = if method == "item/tool/requestUserInput" {
        draft.codex_blocking = Some(params["isBlocking"].as_bool().unwrap_or(true));
        details.push(if params["isBlocking"].as_bool().unwrap_or(true) {
            "Blocking question".into()
        } else {
            "Nonblocking question; Esc returns to your draft".into()
        });
        draft.fields = question_fields(params).unwrap_or_else(|error| {
            draft.unsupported = Some(error.to_string());
            Vec::new()
        });
        (ApprovalRequestKind::CodexQuestion, "Codex needs an answer")
    } else {
        details.insert(
            0,
            format!(
                "MCP server: {}",
                safe_label(params["serverName"].as_str().unwrap_or("unknown"))
            ),
        );
        details.push(safe_text(
            params["message"]
                .as_str()
                .or(params["description"].as_str())
                .unwrap_or(""),
        ));
        build_form(params, &mut draft, &mut details);
        (ApprovalRequestKind::CodexForm, "Codex server request")
    };
    let pending = PendingApprovalRequest {
        request_id: id,
        method: method.into(),
        kind,
        title: title.into(),
        detail_lines: details,
        requested_permissions: Some(params.clone()),
        can_accept_for_session: false,
        can_decline: true,
        can_cancel: true,
    };
    Ok((pending, draft))
}

fn question_fields(params: &Value) -> Result<Vec<AnswerField>> {
    let questions = params["questions"]
        .as_array()
        .filter(|questions| !questions.is_empty() && questions.len() <= 64)
        .context("Expected 1 to64 questions")?;
    let mut seen = HashSet::new();
    let mut fields = Vec::new();
    for question in questions {
        let id = question["id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .context("Question has no native ID")?;
        ensure!(seen.insert(id), "Question IDs repeat");
        question["question"]
            .as_str()
            .context("Question text is missing")?;
        question["header"]
            .as_str()
            .context("Question header is missing")?;
        let mut field = AnswerField::new(
            id,
            json!({"title":question["header"],"description":question["question"],"type":"string"}),
            false,
        );
        field.omitted = false;
        field.secret = question["isSecret"] == true;
        field.options = question["options"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|option| {
                let label = option["label"]
                    .as_str()
                    .context("Choice label is missing")?;
                Ok(AnswerOption {
                    value: json!(label),
                    label: label.into(),
                    description: option["description"].as_str().unwrap_or("").into(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        field.other = !field.options.is_empty();
        field.notes_only = field.other && question["isOther"] != true;
        fields.push(field);
    }
    Ok(fields)
}

fn build_form(params: &Value, draft: &mut NativeDraft, details: &mut Vec<String>) {
    match params["mode"].as_str() {
        Some("form") => {
            let mut schema = params["requestedSchema"].clone();
            if let Some(properties) = schema["properties"].as_object_mut() {
                for value in properties.values_mut() {
                    if let Some(rule) = value.as_object_mut() {
                        rule.retain(|_, value| !value.is_null());
                    }
                }
            }
            if schema.to_string().len() > 65536 {
                draft.unsupported = Some("Form schema exceeds64 KiB".into());
                return;
            }
            draft.fields = mcp_fields(&schema).unwrap_or_else(|error| {
                draft.unsupported = Some(error);
                Vec::new()
            });
            draft.notice = Some("Checking the form schema…".into());
        }
        Some("url") => {
            let url = params["url"].as_str().unwrap_or("");
            match crate::native_form::url_host(url) {
                Ok(host)=>details.splice(0..0,[format!("URL host: {}",safe_label(&host)),format!("Link: {}",safe_label(url)),"Ctrl+Y copies the link; accept consents without confirming completion.".into()]).for_each(drop),
                Err(error)=>draft.unsupported=Some(error.to_string()),
            }
        }
        _ => {
            draft.unsupported = Some(format!(
                "Unsupported request mode: {}",
                safe_label(params["mode"].as_str().unwrap_or("missing"))
            ))
        }
    }
}

/// Preserve native question IDs and return explicit selections/free text as answer arrays.
pub(super) fn question_answer(draft: &NativeDraft) -> Result<Value, String> {
    if let Some(error) = &draft.unsupported {
        return Err(error.clone());
    }
    let mut answers = Map::new();
    for field in &draft.fields {
        let mut values = Vec::new();
        if !field.omitted {
            if field.notes_only && field.selected.is_empty() {
                return Err(format!(
                    "{}: choose a listed answer or Ctrl+O skip",
                    safe_label(&field.title)
                ));
            }
            values.extend(
                field
                    .selected
                    .iter()
                    .map(|index| field.options[*index].value.clone()),
            );
            if field.options.is_empty() || field.use_text {
                let text = field.editor.lines().join("\n");
                if !text.trim().is_empty() {
                    values.push(json!(format!("user_note: {}", text.trim())));
                }
            }
            if values.is_empty() {
                return Err(format!(
                    "{}: choose an answer, enter text, or Ctrl+O skip",
                    safe_label(&field.title)
                ));
            }
        }
        answers.insert(field.name.clone(), json!({"answers":values}));
    }
    Ok(json!({"answers":answers}))
}

/// Build standard MCP content with typed values and explicit optional omissions.
pub(super) fn form_answer(draft: &NativeDraft, params: &Value) -> Result<Value, String> {
    if let Some(error) = &draft.unsupported {
        return Err(error.clone());
    }
    if params["mode"] == "url" {
        return Ok(json!({"action":"accept"}));
    }
    let mut content = Map::new();
    for field in &draft.fields {
        if let Some(value) = field.mcp_value()? {
            content.insert(field.name.clone(), value);
        }
    }
    Ok(json!({"action":"accept","content":content}))
}
