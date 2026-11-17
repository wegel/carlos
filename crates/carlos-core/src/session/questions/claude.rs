//! Parse Claude questions and MCP schemas into plain field descriptions.

use serde_json::{json, Value};

use super::field::{AnswerOption, QuestionField};

/// Describe native Claude questions in the supplied order.
pub(crate) fn ask_fields(input: &Value) -> Result<Vec<QuestionField>, String> {
    let questions = input["questions"]
        .as_array()
        .filter(|q| !q.is_empty())
        .ok_or("No questions supplied")?;
    let mut fields = Vec::new();
    for question in questions {
        let name = question["question"]
            .as_str()
            .ok_or("Question text is missing")?;
        if fields.iter().any(|f: &QuestionField| f.name == name) {
            return Err("Duplicate question text".to_string());
        }
        let mut field = QuestionField::new(
            name,
            json!({ "title": question["header"], "description": name }),
            true,
        );
        field.multiple = question["multiSelect"] == true;
        field.other = true;
        field.options = question["options"]
            .as_array()
            .ok_or("Question choices are missing")?
            .iter()
            .map(|option| {
                let label = option["label"].as_str().ok_or("Choice label is missing")?;
                let description = [
                    option["description"].as_str().unwrap_or(""),
                    option["preview"].as_str().unwrap_or(""),
                ]
                .join("\n");
                Ok(AnswerOption {
                    value: json!(label),
                    label: label.to_string(),
                    description,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        fields.push(field);
    }
    Ok(fields)
}

/// Describe MCP properties without applying defaults to a draft.
pub fn mcp_fields(schema: &Value) -> Result<Vec<QuestionField>, String> {
    carlos_proto::forms::fields(schema)
        .map(|fields| {
            fields
                .into_iter()
                .map(QuestionField::from_input_field)
                .collect()
        })
        .map_err(|error| error.to_string())
}
