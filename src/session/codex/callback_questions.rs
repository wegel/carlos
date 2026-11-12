//! Describe native Codex requests without constructing answer widgets.

use anyhow::{Context, Result};
use serde_json::Value;

use crate::session::approval_state::{ApprovalRequestDraft, ApprovalRequestKind};
use crate::session::questions::{mcp_fields, question_fields, QuestionField};
use crate::session::text::{safe_label, safe_text};

/// Describe the fields and initial notices for one local editor.
pub(crate) struct CallbackEditor {
    pub(crate) fields: Vec<QuestionField>,
    pub(crate) unsupported: Option<String>,
    pub(crate) notice: Option<String>,
    pub(crate) defaults: bool,
}

impl CallbackEditor {
    pub(in crate::session) fn empty() -> Self {
        Self {
            fields: Vec::new(),
            unsupported: None,
            notice: None,
            defaults: false,
        }
    }
}

/// Build a bounded native question/form or an explicit unsupported-mode editor.
pub(crate) fn build(
    method: &str,
    params: &Value,
    id: Value,
) -> Result<(ApprovalRequestDraft, CallbackEditor)> {
    let mut draft = CallbackEditor::empty();
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
        details.push(if params["isBlocking"].as_bool().unwrap_or(true) {
            "Blocking question".into()
        } else {
            "Nonblocking question".into()
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
    let pending = ApprovalRequestDraft {
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

fn build_form(params: &Value, draft: &mut CallbackEditor, details: &mut Vec<String>) {
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
            draft.defaults = true;
            draft.fields = mcp_fields(&schema).unwrap_or_else(|error| {
                draft.unsupported = Some(error);
                Vec::new()
            });
            draft.notice = Some("Checking the form schema…".into());
        }
        Some("url") => {
            let url = params["url"].as_str().unwrap_or("");
            match crate::native_form::url_host(url) {
                Ok(host) => details
                    .splice(
                        0..0,
                        [
                            format!("URL host: {}", safe_label(&host)),
                            format!("Link: {}", safe_label(url)),
                        ],
                    )
                    .for_each(drop),
                Err(error) => draft.unsupported = Some(error.to_string()),
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
