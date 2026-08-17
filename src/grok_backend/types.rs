//! Types, constants, and small helpers for the Grok ACP backend.

use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde_json::{json, Map, Value};

use crate::protocol_params::ModelInfo;

// --- Constants ---

pub(crate) const GROK_PENDING_TURN_ID: &str = "grok-turn-pending";
pub(crate) const GROK_DEFAULT_MODEL: &str = "grok-4.6";
pub(crate) const GROK_DEFAULT_EFFORT: &str = "high";
pub(super) const GROK_DEFAULT_CONTEXT_WINDOW: u64 = 500_000;
const GROK_SUPPORTED_EFFORTS: [&str; 4] = ["xhigh", "high", "medium", "low"];

// --- Launch / runtime ---

#[derive(Debug, Clone)]
pub(crate) enum GrokLaunchMode {
    New,
    Resume(String),
    Continue,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct GrokRuntimeSettings {
    pub(super) model: Option<String>,
    pub(super) effort: Option<String>,
}

impl GrokRuntimeSettings {
    #[cfg(test)]
    pub(crate) fn new(model: Option<String>, effort: Option<String>) -> Self {
        Self { model, effort }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct GrokContextUsage {
    pub(super) used: u64,
    pub(super) max: u64,
}

// --- Translation ---

#[derive(Debug, Clone)]
pub(super) struct GrokToolCall {
    pub(super) name: String,
    pub(super) kind: Option<String>,
    #[allow(dead_code)]
    pub(super) input: Value,
    pub(super) hidden_result: bool,
}

#[derive(Debug, Default)]
pub(crate) struct GrokTranslationState {
    pub(super) replaying: bool,
    pub(super) session_id: Option<String>,
    pub(super) model: Option<String>,
    pub(super) effort: Option<String>,
    pub(super) next_turn_seq: u64,
    pub(super) current_turn_id: Option<String>,
    pub(super) current_message_id: Option<String>,
    pub(super) current_message_text: String,
    pub(super) current_thought_id: Option<String>,
    pub(super) current_thought_text: String,
    pub(super) current_user_text: String,
    pub(super) tools: HashMap<String, GrokToolCall>,
    pub(super) last_usage: Option<GrokContextUsage>,
    pub(super) interrupt_requested: bool,
}

impl GrokTranslationState {
    #[cfg(test)]
    pub(crate) fn replaying() -> Self {
        Self {
            replaying: true,
            ..Self::default()
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct TranslateOutput {
    pub(crate) lines: Vec<String>,
}

// --- Model catalog ---

pub(crate) fn grok_model_catalog() -> Vec<ModelInfo> {
    fallback_model_catalog()
}

pub(super) fn fallback_model_catalog() -> Vec<ModelInfo> {
    vec![
        model_info(GROK_DEFAULT_MODEL, "Grok 4.6", true),
        model_info("grok-4.5", "Grok 4.5", false),
    ]
}

fn model_info(model: &str, display_name: &str, is_default: bool) -> ModelInfo {
    ModelInfo {
        model: model.to_string(),
        display_name: display_name.to_string(),
        supported_efforts: GROK_SUPPORTED_EFFORTS
            .iter()
            .map(|effort| (*effort).to_string())
            .collect(),
        default_effort: Some(GROK_DEFAULT_EFFORT.to_string()),
        is_default,
    }
}

pub(super) fn parse_grok_model_catalog(models: &Value) -> Vec<ModelInfo> {
    let Some(entries) = models.get("availableModels").and_then(Value::as_array) else {
        return fallback_model_catalog();
    };
    let current = models
        .get("currentModelId")
        .and_then(Value::as_str)
        .unwrap_or(GROK_DEFAULT_MODEL);
    let parsed = entries
        .iter()
        .filter_map(|entry| parse_catalog_entry(entry, current))
        .collect::<Vec<_>>();
    if parsed.is_empty() {
        fallback_model_catalog()
    } else {
        parsed
    }
}

fn parse_catalog_entry(entry: &Value, current: &str) -> Option<ModelInfo> {
    let model = entry.get("modelId").and_then(Value::as_str)?.trim();
    if model.is_empty() {
        return None;
    }
    let display_name = entry
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(model)
        .to_string();
    let meta = entry.get("_meta").and_then(Value::as_object);
    let efforts = meta
        .and_then(|meta| meta.get("reasoningEfforts"))
        .and_then(Value::as_array)
        .map(|entries| parse_effort_ids(entries))
        .filter(|efforts| !efforts.is_empty())
        .unwrap_or_else(|| {
            GROK_SUPPORTED_EFFORTS
                .iter()
                .map(|effort| (*effort).to_string())
                .collect()
        });
    let default_effort = meta
        .and_then(|meta| meta.get("reasoningEffort"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|effort| !effort.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| Some(GROK_DEFAULT_EFFORT.to_string()));
    Some(ModelInfo {
        model: model.to_string(),
        display_name,
        supported_efforts: efforts,
        default_effort,
        is_default: model == current,
    })
}

fn parse_effort_ids(entries: &[Value]) -> Vec<String> {
    entries
        .iter()
        .filter_map(|entry| {
            entry
                .get("value")
                .or_else(|| entry.get("id"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
        })
        .collect()
}

// --- Paths ---

pub(super) fn grok_home_dir() -> Option<PathBuf> {
    env::var_os("GROK_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".grok")))
}

pub(super) fn grok_sessions_root() -> Option<PathBuf> {
    grok_home_dir().map(|home| home.join("sessions"))
}

// --- Notification helpers ---

pub(super) fn jsonrpc_notification(method: &str, params: Value) -> String {
    json!({
        "method": method,
        "params": params,
    })
    .to_string()
}

pub(super) fn synthetic_start_response(thread_id: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "result": {
            "thread": { "id": thread_id }
        },
    })
    .to_string()
}

pub(super) fn token_usage_line(usage: GrokContextUsage) -> String {
    jsonrpc_notification(
        "thread/tokenUsage/updated",
        json!({
            "tokenUsage": {
                "modelContextWindow": usage.max,
                "last": { "totalTokens": usage.used },
                "total": { "totalTokens": usage.used },
            }
        }),
    )
}

pub(super) fn normalize_runtime_arg(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("none"))
        .map(ToOwned::to_owned)
}

pub(super) fn content_text(content: &Value) -> Option<String> {
    match content {
        Value::String(text) => {
            let trimmed = text.trim();
            (!trimmed.is_empty()).then(|| text.clone())
        }
        Value::Object(obj) => obj
            .get("text")
            .and_then(Value::as_str)
            .map(str::to_string)
            .filter(|text| !text.is_empty()),
        _ => None,
    }
}

pub(super) fn tool_result_text(update: &Map<String, Value>) -> Option<String> {
    if let Some(text) = update.get("rawOutput").and_then(raw_output_text) {
        return Some(text);
    }
    let contents = update.get("content").and_then(Value::as_array)?;
    let mut parts = Vec::new();
    for entry in contents {
        if let Some(text) = tool_content_entry_text(entry) {
            parts.push(text);
        }
    }
    let joined = parts.join("\n");
    (!joined.trim().is_empty()).then_some(joined)
}

fn raw_output_text(output: &Value) -> Option<String> {
    match output {
        Value::String(text) => (!text.trim().is_empty()).then(|| text.clone()),
        Value::Object(obj) => obj
            .get("output")
            .or_else(|| obj.get("text"))
            .or_else(|| obj.get("stdout"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .filter(|text| !text.trim().is_empty()),
        _ => None,
    }
}

fn tool_content_entry_text(entry: &Value) -> Option<String> {
    if entry.get("type").and_then(Value::as_str) == Some("diff") {
        return format_diff_entry(entry);
    }
    content_text(entry.get("content").unwrap_or(entry))
}

fn format_diff_entry(entry: &Value) -> Option<String> {
    let path = entry.get("path").and_then(Value::as_str).unwrap_or("file");
    let new_text = entry.get("newText").and_then(Value::as_str).unwrap_or("");
    let old_text = entry.get("oldText").and_then(Value::as_str).unwrap_or("");
    let body = if new_text.is_empty() {
        old_text
    } else {
        new_text
    };
    let trimmed = body.trim();
    (!trimmed.is_empty()).then(|| format!("{path}\n{trimmed}"))
}

pub(super) fn should_hide_tool_result(name: &str, kind: Option<&str>) -> bool {
    if kind == Some("read") || kind == Some("search") {
        return true;
    }
    matches!(
        name.trim().to_ascii_lowercase().as_str(),
        "read_file"
            | "read"
            | "grep"
            | "list_dir"
            | "glob"
            | "search_code"
            | "todo_write"
            | "todowrite"
            | "todo_read"
    )
}

pub(super) fn turn_input_text(params: &Value) -> Result<String> {
    params
        .get("input")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("text"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(ToOwned::to_owned)
        .context("missing turn input text")
}

pub(super) fn response_is_error(response: &str) -> bool {
    serde_json::from_str::<Value>(response)
        .ok()
        .and_then(|value| value.get("error").cloned())
        .is_some()
}

pub(super) fn response_error_message(response: &str) -> Option<String> {
    serde_json::from_str::<Value>(response)
        .ok()?
        .get("error")?
        .get("message")?
        .as_str()
        .map(str::to_string)
}
