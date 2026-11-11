//! Builds independent native answer drafts from SDK question payloads.

use std::cell::Cell;
use std::collections::HashMap;

use serde_json::{json, Map, Value};

use super::claude_browser::BrowserRequest;
use super::claude_question_field::{AnswerField, AnswerOption};

/// Unanswered callbacks and consented browser requests retain separate lifetimes.
#[derive(Default)]
pub(super) struct NativeAnswers {
    pub(super) drafts: HashMap<String, NativeDraft>,
    pub(super) browser: HashMap<(String, String), BrowserRequest>,
}

/// One callback owns all answer fields until the SDK accepts or cancels it.
pub(super) struct NativeDraft {
    pub(super) fields: Vec<AnswerField>,
    pub(super) codex_blocking: Option<bool>,
    pub(super) current: usize,
    pub(super) review: bool,
    pub(super) viewport: QuestionViewport,
    pub(super) error: Option<String>,
    pub(super) unsupported: Option<String>,
    pub(super) notice: Option<String>,
}

/// Keep the focused option visible until the user explicitly scrolls the question.
#[derive(Default)]
pub(super) struct QuestionViewport {
    pub(super) scroll: Cell<usize>,
    pub(super) max_scroll: Cell<usize>,
    pub(super) follow_focus: Cell<bool>,
}

impl NativeDraft {
    /// Build answer fields without changing the original SDK callback input.
    pub(super) fn from_payload(kind: &str, payload: &Value) -> Self {
        let mut draft = Self {
            fields: Vec::new(),
            codex_blocking: None,
            current: 0,
            review: false,
            viewport: QuestionViewport::default(),
            error: None,
            unsupported: None,
            notice: None,
        };
        draft.viewport.follow_focus.set(true);
        if kind == "permission"
            && payload["toolName"] == "ExitPlanMode"
            && payload["input"]["plan"]
                .as_str()
                .is_none_or(|plan| plan.trim().is_empty())
        {
            draft.unsupported = Some("Claude supplied no plan text".to_string());
        }
        if kind == "permission" && payload["toolName"] == "AskUserQuestion" {
            draft.fields = ask_fields(&payload["input"]).unwrap_or_else(|error| {
                draft.unsupported = Some(error);
                Vec::new()
            });
        } else if kind == "elicitation" && payload["mode"] != "url" {
            if let Some(error) = payload["formIssue"].as_str() {
                draft.unsupported = Some(error.to_string());
            } else {
                draft.fields = mcp_fields(&payload["requestedSchema"]).unwrap_or_else(|error| {
                    draft.unsupported = Some(error);
                    Vec::new()
                });
            }
        }
        draft
    }

    /// Build native answers, preserving question text and every unrelated input field.
    pub(super) fn answer(&self, payload: &Value) -> Result<Value, String> {
        if let Some(error) = &self.unsupported {
            return Err(format!("Unsupported form: {error}; decline or cancel"));
        }
        if payload["toolName"] == "AskUserQuestion" {
            let mut input = payload["input"].clone();
            let mut answers = Map::new();
            for field in &self.fields {
                answers.insert(field.name.clone(), json!(field.ask_value()?));
            }
            input["answers"] = Value::Object(answers);
            return Ok(json!({ "behavior": "allow", "updatedInput": input }));
        }
        if payload.get("toolName").is_some() {
            return Ok(json!({ "behavior": "allow" }));
        }
        if payload["mode"] == "url" {
            return Ok(json!({ "action": "accept" }));
        }
        let mut content = Map::new();
        for field in &self.fields {
            if let Some(value) = field.mcp_value()? {
                content.insert(field.name.clone(), value);
            }
        }
        Ok(json!({ "action": "accept", "content": content }))
    }

    /// Move between questions and a review page without discarding any drafts.
    pub(super) fn advance(&mut self, backwards: bool) {
        let count = self.fields.len() + 1;
        let index = if self.review {
            self.fields.len()
        } else {
            self.current
        };
        let index = if backwards {
            (index + count - 1) % count
        } else {
            (index + 1) % count
        };
        self.review = index == self.fields.len();
        self.current = index.min(self.fields.len().saturating_sub(1));
        self.viewport.scroll.set(0);
        self.viewport.follow_focus.set(true);
    }
}

fn ask_fields(input: &Value) -> Result<Vec<AnswerField>, String> {
    let questions = input["questions"]
        .as_array()
        .filter(|q| !q.is_empty())
        .ok_or("No questions supplied")?;
    let mut fields = Vec::new();
    for question in questions {
        let name = question["question"]
            .as_str()
            .ok_or("Question text is missing")?;
        if fields.iter().any(|f: &AnswerField| f.name == name) {
            return Err("Duplicate question text".to_string());
        }
        let mut field = AnswerField::new(
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

pub(super) fn mcp_fields(schema: &Value) -> Result<Vec<AnswerField>, String> {
    let properties = schema["properties"]
        .as_object()
        .ok_or("Missing form properties")?;
    let mut fields = Vec::new();
    for (name, rule) in properties {
        let required = schema["required"]
            .as_array()
            .is_some_and(|keys| keys.iter().any(|key| key == name));
        let mut field = AnswerField::new(name, rule.clone(), required);
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
        field.apply_default();
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

/// Escape terminal controls and directional overrides while keeping prose line breaks.
pub(super) fn safe_text(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if (ch.is_control() && ch != '\n')
                || matches!(ch, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
            {
                ch.escape_default().to_string()
            } else {
                ch.to_string()
            }
        })
        .collect()
}

/// Escape line breaks as well as controls in labels, IDs, paths, and server names.
pub(super) fn safe_label(value: &str) -> String {
    safe_text(value).replace('\n', "\\n")
}

/// Keep editing disabled when the terminal can only show the resize/cancel fallback.
pub(super) fn can_edit_question(size: super::TerminalSize) -> bool {
    size.width >= 32 && size.height >= 12
}
