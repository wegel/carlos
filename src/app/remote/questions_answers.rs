//! Build only the offered typed answer from the original local card editor.

use carlos_proto::commands::{ApprovalAnswer, FieldAnswer, FieldAnswers};
use carlos_proto::session::{ApprovalInput, Decision};
use serde_json::{Map, Value};

use crate::app::claude_question_state::NativeDraft;

pub(super) fn answer(input: &ApprovalInput, draft: &NativeDraft) -> Result<ApprovalAnswer, String> {
    if let Some(error) = &draft.unsupported {
        return Err(error.clone());
    }
    let value = match input {
        ApprovalInput::Fields { .. } => fields(draft)?,
        ApprovalInput::Form { schema } => {
            let mut content = Map::new();
            for field in &draft.fields {
                if let Some(value) = field.mcp_value()? {
                    content.insert(field.spec.name().into(), value);
                }
            }
            let value = Value::Object(content);
            carlos_core::native_form::validate(schema, &value)
                .map_err(|error| error.to_string())?;
            value
        }
        ApprovalInput::Browser { .. } => {
            return Ok(ApprovalAnswer::Decision {
                decision: Decision::Once,
            })
        }
        ApprovalInput::Unsupported { reason } => return Err(reason.clone()),
    };
    Ok(ApprovalAnswer::Input { value })
}

fn fields(draft: &NativeDraft) -> Result<Value, String> {
    let mut fields = Vec::new();
    for field in &draft.fields {
        let mut values = Vec::new();
        let mut text = None;
        if !field.omitted {
            values.extend(
                field
                    .selected
                    .iter()
                    .map(|index| field.spec.options()[*index].value().clone()),
            );
            if field.spec.notes_only() && values.is_empty() {
                return Err(format!(
                    "{}: choose a listed answer or Ctrl+O skip",
                    field.spec.title()
                ));
            }
            if field.spec.options().is_empty() || field.use_text {
                let edited = field.editor.lines().join("\n");
                if !edited.trim().is_empty() {
                    text = Some(edited)
                }
            }
            if values.is_empty() && text.is_none() {
                return Err(format!(
                    "{}: choose an answer, enter text, or Ctrl+O skip",
                    field.spec.title()
                ));
            }
        }
        fields.push(FieldAnswer {
            key: field.spec.name().into(),
            values,
            text,
        });
    }
    let value = serde_json::to_value(FieldAnswers { fields }).expect("typed neutral field answer");
    if serde_json::to_vec(&value)
        .expect("typed answer bytes")
        .len()
        > 65536
    {
        return Err("Field answer exceeds the native64KiB bound; your draft is kept".into());
    }
    Ok(value)
}
