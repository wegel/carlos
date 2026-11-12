//! Parse bounded Codex questions while preserving native answer keys.

use std::collections::HashSet;

use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};

use super::field::{AnswerOption, QuestionField};

/// Describe native Codex questions without constructing their editors.
pub(crate) fn question_fields(params: &Value) -> Result<Vec<QuestionField>> {
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
        let mut field = QuestionField::new(
            id,
            json!({"title":question["header"],"description":question["question"],"type":"string"}),
            false,
        );
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
