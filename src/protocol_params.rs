//! Protocol parameter builders and response parsers for the codex app-server JSON-RPC API.

use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use crate::backend::BackendClient;

// --- Data Types ---

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ThreadRuntimeSettings {
    pub(crate) model: Option<String>,
    pub(crate) effort: Option<String>,
    pub(crate) summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModelInfo {
    pub(crate) model: String,
    pub(crate) display_name: String,
    pub(crate) supported_efforts: Vec<String>,
    pub(crate) default_effort: Option<String>,
    pub(crate) is_default: bool,
}

// --- Parameter Builders ---

pub(crate) fn params_initialize() -> Value {
    json!({
        "clientInfo": {
            "name": "carlos",
            "title": "carlos",
            "version": "0.1.0"
        },
        "capabilities": {
            "experimentalApi": true
        }
    })
}

pub(crate) fn params_thread_start(cwd: &str) -> Value {
    json!({
        "experimentalRawEvents": false,
        "persistExtendedHistory": true,
        "cwd": cwd,
    })
}

pub(crate) fn params_thread_resume(thread_id: &str) -> Value {
    json!({
        "threadId": thread_id,
        "persistExtendedHistory": true,
    })
}

pub(crate) fn params_thread_fork(thread_id: &str) -> Value {
    json!({
        "threadId": thread_id,
        "persistExtendedHistory": true,
    })
}

pub(crate) fn params_thread_rollback(thread_id: &str, num_turns: usize) -> Value {
    json!({
        "threadId": thread_id,
        "numTurns": num_turns,
    })
}

pub(crate) fn params_thread_list(cwd: &str) -> Value {
    json!({
        "limit": 100,
        "cwd": cwd,
    })
}

pub(crate) fn params_thread_archive(thread_id: &str) -> Value {
    json!({
        "threadId": thread_id,
    })
}

pub(crate) fn params_model_list(cursor: Option<&str>) -> Value {
    let mut params = json!({
        "includeHidden": false,
        "limit": 200,
    });
    if let Some(cursor) = cursor.filter(|c| !c.trim().is_empty()) {
        params["cursor"] = Value::String(cursor.to_string());
    }
    params
}

pub(crate) fn params_turn_start(
    thread_id: &str,
    text: &str,
    model: Option<&str>,
    effort: Option<&str>,
    summary: Option<&str>,
) -> Value {
    let mut params = json!({
        "threadId": thread_id,
        "input": [{
            "type": "text",
            "text": text,
            "text_elements": []
        }]
    });
    if let Some(model) = model.filter(|m| !m.trim().is_empty()) {
        params["model"] = Value::String(model.to_string());
    }
    if let Some(effort) = effort.filter(|e| !e.trim().is_empty()) {
        params["effort"] = Value::String(effort.to_string());
    }
    if let Some(summary) = summary.filter(|s| !s.trim().is_empty()) {
        params["summary"] = Value::String(summary.to_string());
    }
    params
}

pub(crate) fn params_turn_steer(thread_id: &str, turn_id: &str, text: &str) -> Value {
    json!({
        "threadId": thread_id,
        "expectedTurnId": turn_id,
        "input": [{
            "type": "text",
            "text": text,
            "text_elements": []
        }]
    })
}

pub(crate) fn params_turn_interrupt(thread_id: &str, turn_id: &str) -> Value {
    json!({
        "threadId": thread_id,
        "turnId": turn_id,
    })
}

// --- Client Initialisation ---

pub(crate) fn initialize_client(client: &dyn BackendClient) -> Result<()> {
    let resp = client.call("initialize", params_initialize(), Duration::from_secs(10))?;
    extract_result_object(&resp)?;
    Ok(())
}

// --- Response Parsers ---

pub(crate) fn extract_result_object(line: &str) -> Result<Value> {
    let parsed: Value = serde_json::from_str(line).context("invalid JSON response")?;
    if parsed.get("error").is_some() {
        bail!(
            "server returned error: {}",
            parsed.get("error").unwrap_or(&Value::Null)
        );
    }
    if parsed.get("result").is_none() {
        bail!("missing result in response");
    }
    Ok(parsed)
}

pub(crate) fn parse_thread_id_from_start_or_resume(response_line: &str) -> Result<String> {
    let parsed = extract_result_object(response_line)?;
    let result = parsed
        .get("result")
        .and_then(Value::as_object)
        .context("invalid result object")?;
    let thread = result
        .get("thread")
        .and_then(Value::as_object)
        .context("missing thread in result")?;
    let id = thread
        .get("id")
        .and_then(Value::as_str)
        .context("missing thread.id")?;
    Ok(id.to_string())
}

pub(crate) fn parse_thread_runtime_settings(response_line: &str) -> Result<ThreadRuntimeSettings> {
    let parsed = extract_result_object(response_line)?;
    let result = parsed
        .get("result")
        .and_then(Value::as_object)
        .context("invalid result object")?;

    let model = result
        .get("model")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned);

    let effort = result
        .get("reasoningEffort")
        .or_else(|| result.get("effort"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned);

    let summary = result
        .get("summary")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned);

    Ok(ThreadRuntimeSettings {
        model,
        effort,
        summary,
    })
}

pub(crate) fn parse_model_list_page(
    response_line: &str,
) -> Result<(Vec<ModelInfo>, Option<String>)> {
    let parsed = extract_result_object(response_line)?;
    let result = parsed
        .get("result")
        .and_then(Value::as_object)
        .context("invalid result object")?;

    let mut out = Vec::new();
    if let Some(data) = result.get("data").and_then(Value::as_array) {
        for item in data {
            let Some(obj) = item.as_object() else {
                continue;
            };
            let Some(model) = obj.get("model").and_then(Value::as_str) else {
                continue;
            };
            if model.trim().is_empty() {
                continue;
            }
            let display_name = obj
                .get("displayName")
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty())
                .unwrap_or(model)
                .to_string();
            let is_default = obj
                .get("isDefault")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let default_effort = obj
                .get("defaultReasoningEffort")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned);
            let supported_efforts = obj
                .get("supportedReasoningEfforts")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_object())
                        .filter_map(|o| o.get("reasoningEffort").and_then(Value::as_str))
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(ToOwned::to_owned)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            out.push(ModelInfo {
                model: model.to_string(),
                display_name,
                supported_efforts,
                default_effort,
                is_default,
            });
        }
    }

    let next_cursor = result
        .get("nextCursor")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned);

    Ok((out, next_cursor))
}
