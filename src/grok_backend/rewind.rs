//! Grok ACP rewind: list points, map Carlos history indexes, execute truncate.

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};

use crate::backend::RewindForkRequest;

use super::types::synthetic_start_response;
use super::GrokClient;

const REWIND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

// --- Types ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GrokRewindPoint {
    pub(crate) prompt_index: u64,
    pub(crate) prompt_preview: String,
}

// --- Client API ---

impl GrokClient {
    pub(super) fn rewind_fork(&self, request: RewindForkRequest) -> Result<String> {
        let source_id = self.current_session_id()?;
        let points = self.list_rewind_points(&source_id)?;
        let target = resolve_rewind_target(&points, request.keep_turns)?;
        let session_id = match self.fork_session(&source_id) {
            Ok(new_id) => new_id,
            Err(err) if is_method_not_found(&err.to_string()) => source_id,
            Err(err) => return Err(err),
        };
        self.execute_rewind(&session_id, target)?;
        self.store_session_id(session_id.clone());
        Ok(synthetic_start_response(&session_id))
    }

    fn list_rewind_points(&self, session_id: &str) -> Result<Vec<GrokRewindPoint>> {
        let response = self.session_request(
            "_x.ai/rewind/points",
            json!({ "sessionId": session_id }),
            REWIND_TIMEOUT,
        )?;
        let result = rpc_result(&response).context("Grok rewind/points failed")?;
        Ok(parse_rewind_points(&result))
    }

    fn fork_session(&self, source_id: &str) -> Result<String> {
        let cwd = self.cwd.to_string_lossy();
        let response = self.session_request(
            "_x.ai/session/fork",
            json!({
                "sourceSessionId": source_id,
                "sourceCwd": cwd,
                "newCwd": cwd,
            }),
            REWIND_TIMEOUT,
        )?;
        let result = rpc_result(&response).context("Grok session/fork failed")?;
        result
            .get("newSessionId")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(ToOwned::to_owned)
            .context("Grok session/fork returned no newSessionId")
    }

    fn execute_rewind(&self, session_id: &str, target_prompt_index: u64) -> Result<()> {
        let response = self.session_request(
            "_x.ai/rewind/execute",
            json!({
                "sessionId": session_id,
                "targetPromptIndex": target_prompt_index,
                "mode": "conversation_only",
                "force": true,
            }),
            REWIND_TIMEOUT,
        )?;
        let result = rpc_result(&response).context("Grok rewind/execute failed")?;
        if result.get("success").and_then(Value::as_bool) == Some(false) {
            let detail = result
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("rewind did not truncate");
            bail!("{detail}");
        }
        Ok(())
    }
}

// --- Mapping ---

pub(crate) fn resolve_rewind_target(points: &[GrokRewindPoint], keep_turns: usize) -> Result<u64> {
    let facing = user_facing_rewind_points(points);
    let target = facing.get(keep_turns).with_context(|| {
        format!(
            "Grok rewind target {keep_turns} is out of range ({} user prompts)",
            facing.len()
        )
    })?;
    Ok(target.prompt_index)
}

pub(crate) fn parse_rewind_points(payload: &Value) -> Vec<GrokRewindPoint> {
    let list = unwrap_rewind_list(payload);
    list.iter().filter_map(parse_rewind_point).collect()
}

fn unwrap_rewind_list(payload: &Value) -> &[Value] {
    if let Some(list) = payload.as_array() {
        return list;
    }
    let obj = payload.as_object();
    let nested = obj
        .and_then(|obj| obj.get("result"))
        .and_then(Value::as_object)
        .or(obj);
    nested
        .and_then(|obj| {
            obj.get("rewind_points")
                .or_else(|| obj.get("rewindPoints"))
                .or_else(|| obj.get("points"))
        })
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn parse_rewind_point(value: &Value) -> Option<GrokRewindPoint> {
    let prompt_index = value
        .get("prompt_index")
        .or_else(|| value.get("promptIndex"))
        .and_then(Value::as_u64)?;
    let preview = value
        .get("prompt_preview")
        .or_else(|| value.get("promptPreview"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    Some(GrokRewindPoint {
        prompt_index,
        prompt_preview: preview,
    })
}

pub(crate) fn user_facing_rewind_points(points: &[GrokRewindPoint]) -> Vec<&GrokRewindPoint> {
    points
        .iter()
        .filter(|point| !is_hidden_rewind_point(&point.prompt_preview))
        .collect()
}

pub(crate) fn is_hidden_rewind_point(preview: &str) -> bool {
    let trimmed = preview.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.starts_with("<system-reminder>") {
        return true;
    }
    matches!(
        trimmed.to_ascii_lowercase().as_str(),
        "[plan approved]" | "[plan rejected]" | "[plan cancelled]"
    )
}

fn rpc_result(response: &str) -> Result<Value> {
    let parsed: Value = serde_json::from_str(response).context("invalid Grok JSON-RPC")?;
    if let Some(error) = parsed.get("error") {
        let code = error.get("code").and_then(Value::as_i64).unwrap_or(0);
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("unknown error");
        if code == -32601 {
            return Err(anyhow!("method not found: {message}"));
        }
        bail!("{message}");
    }
    Ok(parsed.get("result").cloned().unwrap_or(Value::Null))
}

fn is_method_not_found(message: &str) -> bool {
    message.contains("method not found")
}
