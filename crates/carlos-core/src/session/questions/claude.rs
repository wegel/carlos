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
    let properties = schema["properties"]
        .as_object()
        .ok_or("Missing form properties")?;
    let mut fields = Vec::new();
    for (name, rule) in properties {
        let required = schema["required"]
            .as_array()
            .is_some_and(|keys| keys.iter().any(|key| key == name));
        let mut field = QuestionField::new(name, rule.clone(), required);
        field.options = if rule["type"] == "boolean" {
            vec![
                AnswerOption {
                    value: json!(true),
                    label: "true".to_string(),
                    description: String::new(),
                },
                AnswerOption {
                    value: json!(false),
                    label: "false".to_string(),
                    description: String::new(),
                },
            ]
        } else {
            schema_options(if field.multiple { &rule["items"] } else { rule })?
        };
        fields.push(field);
    }
    Ok(fields)
}

fn schema_options(rule: &Value) -> Result<Vec<AnswerOption>, String> {
    if let Some(options) = rule["enum"].as_array() {
        return options
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let label = rule["enumNames"][index]
                    .as_str()
                    .or(value.as_str())
                    .ok_or("Non-string choice")?;
                Ok(AnswerOption {
                    value: value.clone(),
                    label: label.to_string(),
                    description: String::new(),
                })
            })
            .collect();
    }
    let options = rule["oneOf"]
        .as_array()
        .or_else(|| rule["anyOf"].as_array());
    Ok(options
        .map(|options| {
            options
                .iter()
                .map(|option| AnswerOption {
                    value: option["const"].clone(),
                    label: option["title"].as_str().unwrap_or("").to_string(),
                    description: String::new(),
                })
                .collect()
        })
        .unwrap_or_default())
}
