//! Describe a Claude question before a client creates its answer fields.

use serde_json::Value;

use crate::session::questions::{ask_fields, mcp_fields, QuestionField};

/// Describe backend fields and unsupported requests without an editable answer.
pub(crate) struct ClaudeQuestion {
    pub(crate) fields: Vec<QuestionField>,
    pub(crate) unsupported: Option<String>,
    pub(crate) defaults: bool,
}

impl ClaudeQuestion {
    /// Keep the native request checks and error precedence used by the terminal client.
    pub(crate) fn from_payload(kind: &str, payload: &Value) -> Self {
        let mut description = Self {
            fields: Vec::new(),
            unsupported: None,
            defaults: false,
        };
        if kind == "permission"
            && payload["toolName"] == "ExitPlanMode"
            && payload["input"]["plan"]
                .as_str()
                .is_none_or(|plan| plan.trim().is_empty())
        {
            description.unsupported = Some("Claude supplied no plan text".to_string());
        }
        let fields = if kind == "permission" && payload["toolName"] == "AskUserQuestion" {
            Some(ask_fields(&payload["input"]))
        } else if kind == "elicitation" && payload["mode"] != "url" {
            if let Some(error) = payload["formIssue"].as_str() {
                description.unsupported = Some(error.to_string());
                None
            } else {
                description.defaults = true;
                Some(mcp_fields(&payload["requestedSchema"]))
            }
        } else {
            None
        };
        if let Some(fields) = fields {
            match fields {
                Ok(fields) => description.fields = fields,
                Err(error) => description.unsupported = Some(error),
            }
        }
        description
    }
}
