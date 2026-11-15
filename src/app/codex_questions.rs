//! Build Codex question editors and native replies without changing prompt text.

use carlos_core::session::text::safe_label;
use serde_json::{json, Map, Value};

use super::claude_question_field::AnswerField;
use super::claude_question_state::NativeDraft;

/// Create editors once, applying defaults only for MCP fields.
pub(super) fn from_description(
    editor: carlos_core::session::codex::callback_questions::CallbackEditor,
) -> NativeDraft {
    let mut draft = NativeDraft::from_payload("", &Value::Null);
    draft.fields = editor
        .fields
        .into_iter()
        .map(|description| {
            let mut field = AnswerField::from_description(description);
            if editor.defaults {
                field.apply_default();
            } else {
                field.omitted = false;
            }
            field
        })
        .collect();
    draft.unsupported = editor.unsupported;
    draft.notice = editor.notice;
    draft
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
            if field.spec.notes_only() && field.selected.is_empty() {
                return Err(format!(
                    "{}: choose a listed answer or Ctrl+O skip",
                    safe_label(field.spec.title())
                ));
            }
            values.extend(
                field
                    .selected
                    .iter()
                    .map(|index| field.spec.options()[*index].value().clone()),
            );
            if field.spec.options().is_empty() || field.use_text {
                let text = field.editor.lines().join("\n");
                if !text.trim().is_empty() {
                    values.push(json!(format!("user_note: {}", text.trim())));
                }
            }
            if values.is_empty() {
                return Err(format!(
                    "{}: choose an answer, enter text, or Ctrl+O skip",
                    safe_label(field.spec.title())
                ));
            }
        }
        answers.insert(field.spec.name().to_string(), json!({"answers":values}));
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
            content.insert(field.spec.name().to_string(), value);
        }
    }
    Ok(json!({"action":"accept","content":content}))
}
