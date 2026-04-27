//! Claude token-usage parsing and synthetic notification helpers.

// --- Imports ---

use serde_json::{json, Map, Value};

use super::types::normalize_claude_model_name;

// --- Types ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ClaudeContextUsage {
    pub(crate) used: u64,
    pub(crate) max: u64,
}

// --- Context window helpers ---

fn context_window_from_model_hint(raw_model: &str) -> Option<u64> {
    let suffix = raw_model
        .trim()
        .rsplit_once('[')
        .and_then(|(_, suffix)| suffix.strip_suffix(']'))?;
    let lower = suffix.trim().to_ascii_lowercase();
    if let Some(value) = lower.strip_suffix('m') {
        return value.parse::<u64>().ok().map(|n| n * 1_000_000);
    }
    if let Some(value) = lower.strip_suffix('k') {
        return value.parse::<u64>().ok().map(|n| n * 1_000);
    }
    lower.parse::<u64>().ok()
}

// --- Usage parsing ---

fn sum_snake_usage_tokens(usage: &Map<String, Value>) -> u64 {
    [
        usage.get("input_tokens").and_then(Value::as_u64),
        usage
            .get("cache_creation_input_tokens")
            .and_then(Value::as_u64),
        usage.get("cache_read_input_tokens").and_then(Value::as_u64),
        usage.get("output_tokens").and_then(Value::as_u64),
    ]
    .into_iter()
    .flatten()
    .sum()
}

fn sum_camel_usage_tokens(usage: &Map<String, Value>) -> u64 {
    [
        usage.get("inputTokens").and_then(Value::as_u64),
        usage
            .get("cacheCreationInputTokens")
            .and_then(Value::as_u64),
        usage.get("cacheReadInputTokens").and_then(Value::as_u64),
        usage.get("outputTokens").and_then(Value::as_u64),
    ]
    .into_iter()
    .flatten()
    .sum()
}

pub(super) fn claude_context_usage_from_usage(
    usage: &Map<String, Value>,
    raw_model: Option<&str>,
    _model: Option<&str>,
) -> Option<ClaudeContextUsage> {
    let used = sum_snake_usage_tokens(usage);
    let max = raw_model.and_then(context_window_from_model_hint)?;
    (max > 0).then_some(ClaudeContextUsage {
        used: used.min(max),
        max,
    })
}

pub(super) fn claude_context_usage_from_model_usage(
    model_usage: &Map<String, Value>,
) -> Option<ClaudeContextUsage> {
    model_usage
        .values()
        .filter_map(Value::as_object)
        .filter_map(|entry| {
            let max = entry.get("contextWindow").and_then(Value::as_u64)?;
            (max > 0).then_some(ClaudeContextUsage {
                used: sum_camel_usage_tokens(entry).min(max),
                max,
            })
        })
        .max_by_key(|usage| usage.used)
}

pub(super) fn claude_context_usage_from_record(record: &Value) -> Option<ClaudeContextUsage> {
    if let Some(model_usage) = record.get("modelUsage").and_then(Value::as_object) {
        if let Some(usage) = claude_context_usage_from_model_usage(model_usage) {
            return Some(usage);
        }
    }
    if let Some(model_usage) = record
        .get("message")
        .and_then(Value::as_object)
        .and_then(|message| message.get("modelUsage"))
        .and_then(Value::as_object)
    {
        if let Some(usage) = claude_context_usage_from_model_usage(model_usage) {
            return Some(usage);
        }
    }

    let raw_model = record
        .get("model")
        .and_then(Value::as_str)
        .or_else(|| {
            record
                .get("message")
                .and_then(Value::as_object)
                .and_then(|message| message.get("model"))
                .and_then(Value::as_str)
        });
    let model = raw_model.map(normalize_claude_model_name);

    record
        .get("usage")
        .and_then(Value::as_object)
        .and_then(|usage| {
            claude_context_usage_from_usage(usage, raw_model, model.as_deref())
        })
        .or_else(|| {
            record
                .get("message")
                .and_then(Value::as_object)
                .and_then(|message| message.get("usage"))
                .and_then(Value::as_object)
                .and_then(|usage| {
                    claude_context_usage_from_usage(usage, raw_model, model.as_deref())
                })
        })
}

// --- Notification synthesis ---

pub(super) fn synthetic_token_usage_line_from_context_usage(
    usage: ClaudeContextUsage,
) -> String {
    json!({
        "method": "thread/tokenUsage/updated",
        "params": {
            "tokenUsage": {
                "modelContextWindow": usage.max,
                "last": {
                    "totalTokens": usage.used,
                },
                "total": {
                    "totalTokens": usage.used,
                }
            }
        }
    })
    .to_string()
}

pub(super) fn synthetic_token_usage_line(
    usage: &Map<String, Value>,
    raw_model: Option<&str>,
    model: Option<&str>,
) -> Option<String> {
    claude_context_usage_from_usage(usage, raw_model, model)
        .map(synthetic_token_usage_line_from_context_usage)
}

pub(super) fn synthetic_token_usage_line_from_model_usage(
    model_usage: &Map<String, Value>,
) -> Option<String> {
    claude_context_usage_from_model_usage(model_usage)
        .map(synthetic_token_usage_line_from_context_usage)
}
