//! Formats native question fields and caller metadata as safe terminal text.

use serde_json::Value;

use super::claude_question_field::AnswerField;
use super::claude_question_state::{safe_label, safe_text, NativeDraft};
use super::state::PendingApprovalRequest;
use super::text::{visual_width, wrap_natural_by_cells};

/// Name the actual approval key without treating answer text as consent.
pub(super) fn default_no_hint(has_fields: bool, again: bool) -> &'static str {
    match (has_fields, again) {
        (true, false) => "Press Ctrl+S twice to send answers; Alt+N declines",
        (true, true) => "Press Ctrl+S again to send answers; Alt+N declines",
        (false, false) => "Press y twice to allow this request; n declines",
        (false, true) => "Press y again to allow; n declines",
    }
}

/// Keep the requesting server or child tool visible above the answer field.
pub(super) fn caller(pending: &PendingApprovalRequest) -> String {
    let payload = pending
        .requested_permissions
        .as_ref()
        .unwrap_or(&Value::Null);
    if payload["threadId"].is_string() && pending.method != "claude/native" {
        let thread = payload["threadId"].as_str().unwrap_or("unknown");
        let tail = thread
            .chars()
            .rev()
            .take(8)
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>();
        return format!(
            "Thread: …{}  {}",
            safe_label(&tail),
            safe_label(payload["serverName"].as_str().unwrap_or("Codex question"))
        );
    }
    if let Some(server) = payload["serverName"].as_str() {
        return format!("MCP server: {}", safe_label(server));
    }
    let mut label = format!(
        "Tool: {}",
        safe_label(payload["toolName"].as_str().unwrap_or("unknown"))
    );
    if let Some(agent) = payload["agentID"].as_str() {
        label.push_str(&format!("  Child: {}", safe_label(agent)));
    }
    label
}

/// Show the current editor line with enough horizontal room for its caret.
pub(super) fn editor_line(draft: &NativeDraft, width: usize) -> Option<(String, usize)> {
    if draft.review {
        return None;
    }
    let field = draft.fields.get(draft.current)?;
    if !field.options.is_empty() && !(field.other && field.cursor == field.options.len()) {
        return None;
    }
    let (row, col) = field.editor.cursor();
    let line = field.editor.lines().get(row)?;
    let prefix = if field.secret {
        "•".repeat(col)
    } else {
        safe_text(&line.chars().take(col).collect::<String>())
    };
    let suffix = if field.secret {
        "•".repeat(line.chars().count().saturating_sub(col))
    } else {
        safe_text(&line.chars().skip(col).collect::<String>())
    };
    let mut visible = prefix.as_str();
    while visual_width(visible) > width.saturating_sub(3) {
        visible = &visible[visible.chars().next()?.len_utf8()..];
    }
    Some((format!("› {visible}{suffix}"), 2 + visual_width(visible)))
}

/// Describe answers and metadata without displaying raw terminal control bytes.
pub(super) fn body_lines(pending: &PendingApprovalRequest, draft: &NativeDraft) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(notice) = &draft.notice {
        lines.push(notice.clone());
    }
    if let Some(error) = &draft.error {
        lines.push(format!("Answer error: {}", safe_text(error)));
    }
    if let Some(error) = &draft.unsupported {
        lines.push(format!(
            "Cannot accept: {}. Decline or cancel.",
            safe_text(error)
        ));
    }
    if !draft.fields.is_empty() {
        if draft.review {
            lines.extend(review_lines(draft));
        } else {
            lines.push(format!(
                "Question {}/{}",
                draft.current + 1,
                draft.fields.len()
            ));
            lines.extend(field_lines(&draft.fields[draft.current]));
        }
    }
    lines.push(format!(
        "Request: {}",
        super::native_questions::id_label(&pending.request_id)
    ));
    lines.extend(pending.detail_lines.iter().cloned());
    lines
}

/// Keep compact answer keys on screen even on a short phone terminal.
pub(super) fn footer_lines(
    draft: &NativeDraft,
    pending: &PendingApprovalRequest,
    width: usize,
    compact: bool,
) -> Vec<String> {
    if draft.unsupported.is_some() {
        let question = draft.codex_blocking.is_some();
        return wrap_lines(
            vec![
                if question {
                    "Alt+N skip  Alt+C interrupt".into()
                } else {
                    "Alt+N decline  Alt+C cancel".into()
                },
                "F4 next  F10 interrupt".into(),
                "PgUp/PgDn scroll".into(),
            ],
            width,
        );
    }
    if let Some(blocking) = draft.codex_blocking {
        return vec![
            "Tab fields ↑↓/Space pick F4 next".into(),
            "Ctrl+S send Alt+N skip".into(),
            if blocking {
                "Alt+C/F10 interrupt".into()
            } else {
                "Alt+C/F10 interrupt; Esc back".into()
            },
        ];
    }
    if draft.fields.is_empty() && pending.method != "claude/native" {
        let mut choices = Vec::new();
        if pending.can_accept_once() {
            choices.push("y accept");
        }
        if pending.can_accept_for_session {
            choices.push("s accept session");
        }
        if pending.can_decline {
            choices.push("n decline");
        }
        if pending.can_cancel {
            choices.push("c cancel");
        }
        let mut lines = wrap_lines(vec![choices.join("  ")], width);
        lines.push("F4 next  F10 interrupt".into());
        lines.push("PgUp/PgDn scroll".into());
        return lines;
    }
    if compact {
        return vec![
            "Tab fields ↑↓/Space pick F4 next".to_string(),
            "Ctrl+S send Alt+N no".to_string(),
            "Alt+C cancel F10 interrupt".to_string(),
        ];
    }
    let mut lines = if draft.fields.is_empty() {
        vec!["y allow/consent  n decline  c cancel".to_string()]
    } else {
        vec![
            "Tab next  Shift+Tab back".to_string(),
            "↑↓ choose  Space mark  Enter next".to_string(),
            "Ctrl+S send  Alt+N decline  Alt+C cancel".to_string(),
        ]
    };
    lines.push("F4 next request  F10 interrupt".to_string());
    lines.push("Ctrl+M settings  PgUp/PgDn scroll".to_string());
    wrap_lines(lines, width)
}

/// Wrap prose while keeping empty lines and explicit line breaks.
pub(super) fn wrap_lines(lines: Vec<String>, width: usize) -> Vec<String> {
    lines
        .iter()
        .flat_map(|line| {
            line.split('\n').flat_map(|part| {
                let wrapped = wrap_natural_by_cells(part, width);
                if wrapped.is_empty() {
                    vec![String::new()]
                } else {
                    wrapped
                }
            })
        })
        .collect()
}

fn review_lines(draft: &NativeDraft) -> Vec<String> {
    let mut lines = vec!["Review answers. Ctrl+S sends; Tab edits.".to_string()];
    for field in &draft.fields {
        if field.secret {
            lines.push(format!("{}: (hidden)", safe_label(&field.title)));
            continue;
        }
        let value = if field.other {
            field.ask_value().map(|v| Some(serde_json::json!(v)))
        } else {
            field.mcp_value()
        };
        lines.push(format!(
            "{}: {}",
            safe_label(&field.title),
            safe_text(&match value {
                Ok(Some(value)) => value.to_string(),
                Ok(None) => "(omitted)".to_string(),
                Err(error) => error,
            })
        ));
    }
    lines
}

fn field_lines(field: &AnswerField) -> Vec<String> {
    let mut lines = vec![
        format!(
            "{}{}",
            safe_label(&field.title),
            if field.required {
                " (required)"
            } else {
                " (optional, Ctrl+O omit)"
            }
        ),
        safe_text(&field.description),
    ];
    for (index, option) in field.options.iter().enumerate() {
        lines.push(format!(
            "{} [{}] {}",
            if field.cursor == index { ">" } else { " " },
            if field.selected.contains(&index) {
                "x"
            } else {
                " "
            },
            safe_label(&option.label)
        ));
        if field.cursor == index && !option.description.trim().is_empty() {
            lines.push(format!("  {}", safe_text(&option.description)));
        }
    }
    if field.other {
        lines.push(format!(
            "{} [{}] {}",
            if field.cursor == field.options.len() {
                ">"
            } else {
                " "
            },
            if field.use_text { "x" } else { " " },
            if field.notes_only {
                "Notes (optional)"
            } else {
                "Other (type your answer)"
            }
        ));
    }
    if field.options.is_empty() && !field.other {
        lines.push(field_guidance(field));
    }
    if field.schema["type"] == "array" {
        lines.push(field_guidance(field));
    }
    if field.omitted {
        lines.push("This optional field will be omitted.".to_string());
    }
    lines
}

fn field_guidance(field: &AnswerField) -> String {
    let mut parts = vec![format!(
        "Enter {}",
        field.schema["type"].as_str().unwrap_or("text")
    )];
    for (key, label) in [
        ("minLength", "min characters"),
        ("maxLength", "max characters"),
        ("minimum", "minimum"),
        ("maximum", "maximum"),
        ("format", "format"),
        ("pattern", "pattern"),
        ("minItems", "minimum choices"),
        ("maxItems", "maximum choices"),
    ] {
        if let Some(value) = field.schema.get(key) {
            parts.push(format!(
                "{label}: {}",
                safe_text(value.as_str().unwrap_or(&value.to_string()))
            ));
        }
    }
    parts.join("; ")
}
