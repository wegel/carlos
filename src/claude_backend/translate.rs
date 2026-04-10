//! Claude CLI stream event translation into the codex notification protocol.

use anyhow::{Context, Result};
use serde_json::{json, Map, Value};

use super::exit_plan::{
    claude_exit_plan_approval_from_tool_call, claude_exit_plan_request_line,
    synthetic_tool_result_line,
};
use super::snapshot::synthesize_assistant_snapshot;
use super::types::{
    begin_claude_message, ensure_claude_turn_started, normalize_claude_model_name,
    parse_partial_json_object, should_hide_claude_tool_transcript, synthetic_token_usage_line,
    ClaudeBlockState, ClaudeToolCall, ClaudeTranslationState, TranslateOutput,
};

// --- Event translation ---

pub(crate) fn translate_claude_line(
    state: &mut ClaudeTranslationState,
    line: &str,
) -> Result<TranslateOutput> {
    let parsed: Value = serde_json::from_str(line).context("invalid Claude JSON line")?;
    let root = parsed.as_object().context("expected Claude JSON object")?;

    let mut out = TranslateOutput::default();
    match root.get("type").and_then(Value::as_str) {
        Some("system") if root.get("subtype").and_then(Value::as_str) == Some("init") => {
            translate_system_init(state, root, &mut out)?;
        }
        Some("stream_event") => {
            translate_stream_event(state, root, &mut out)?;
        }
        Some("assistant") => {
            translate_assistant_record(state, root, &mut out);
        }
        Some("user") => {
            translate_user_record(state, root, &mut out);
        }
        Some("result") => {
            translate_result_record(state, root, &mut out);
        }
        _ => {}
    }

    Ok(out)
}

// --- Per-event handlers ---

fn translate_system_init(
    state: &mut ClaudeTranslationState,
    root: &Map<String, Value>,
    out: &mut TranslateOutput,
) -> Result<()> {
    let session_id = root
        .get("session_id")
        .and_then(Value::as_str)
        .context("missing Claude session_id")?;
    let model = root
        .get("model")
        .and_then(Value::as_str)
        .map(normalize_claude_model_name);
    state.session_id = Some(session_id.to_string());
    state.model = model.clone();
    let mut params = Map::new();
    params.insert("thread".to_string(), json!({ "id": session_id }));
    if let Some(model) = state.model.as_deref() {
        params.insert("model".to_string(), Value::String(model.to_string()));
    }
    out.lines.push(
        json!({
            "method": "thread/initialized",
            "params": Value::Object(params),
        })
        .to_string(),
    );
    Ok(())
}

fn translate_stream_event(
    state: &mut ClaudeTranslationState,
    root: &Map<String, Value>,
    out: &mut TranslateOutput,
) -> Result<()> {
    let event = root
        .get("event")
        .and_then(Value::as_object)
        .context("missing Claude stream event")?;
    match event.get("type").and_then(Value::as_str) {
        Some("message_start") => {
            ensure_claude_turn_started(state, out);
            begin_claude_message(state);
        }
        Some("content_block_start") => {
            translate_content_block_start(state, event, out)?;
        }
        Some("content_block_delta") => {
            translate_content_block_delta(state, event, out)?;
        }
        Some("content_block_stop") => {
            translate_content_block_stop(state, event, out)?;
        }
        Some("message_delta") => {
            if let Some(usage) = event.get("usage").and_then(Value::as_object) {
                out.lines
                    .push(synthetic_token_usage_line(usage, state.model.as_deref()));
            }
        }
        Some("message_stop") => {}
        _ => {}
    }
    Ok(())
}

fn translate_content_block_start(
    state: &mut ClaudeTranslationState,
    event: &Map<String, Value>,
    out: &mut TranslateOutput,
) -> Result<()> {
    let index = event
        .get("index")
        .and_then(Value::as_u64)
        .context("missing Claude content block index")? as usize;
    let block = event
        .get("content_block")
        .and_then(Value::as_object)
        .context("missing Claude content_block")?;
    match block.get("type").and_then(Value::as_str) {
        Some("text") => {
            state.current_message_has_content_blocks = true;
            let item_id = format!("claude-msg-{}-{}", state.current_message_seq, index);
            state.current_blocks.insert(
                index,
                ClaudeBlockState::Text {
                    item_id: item_id.clone(),
                    text: String::new(),
                },
            );
            out.lines.push(
                json!({
                    "method": "item/started",
                    "params": {
                        "item": {
                            "id": item_id,
                            "type": "agentMessage"
                        }
                    }
                })
                .to_string(),
            );
        }
        Some("tool_use") => {
            state.current_message_has_content_blocks = true;
            let item_id = block
                .get("id")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| {
                    format!("claude-tool-{}-{}", state.current_message_seq, index)
                });
            let name = block
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("Tool")
                .to_string();
            state.current_blocks.insert(
                index,
                ClaudeBlockState::ToolUse {
                    item_id: item_id.clone(),
                    name,
                    input_json: String::new(),
                },
            );
        }
        _ => {}
    }
    Ok(())
}

fn translate_content_block_delta(
    state: &mut ClaudeTranslationState,
    event: &Map<String, Value>,
    out: &mut TranslateOutput,
) -> Result<()> {
    let index = event
        .get("index")
        .and_then(Value::as_u64)
        .context("missing Claude delta index")? as usize;
    let delta = event
        .get("delta")
        .and_then(Value::as_object)
        .context("missing Claude delta")?;
    match state.current_blocks.get_mut(&index) {
        Some(ClaudeBlockState::Text { item_id, text }) => {
            if delta.get("type").and_then(Value::as_str) == Some("text_delta") {
                let fragment = delta.get("text").and_then(Value::as_str).unwrap_or("");
                text.push_str(fragment);
                out.lines.push(
                    json!({
                        "method": "item/agentMessage/delta",
                        "params": {
                            "itemId": item_id,
                            "delta": fragment,
                        }
                    })
                    .to_string(),
                );
            }
        }
        Some(ClaudeBlockState::ToolUse { input_json, .. }) => {
            if delta.get("type").and_then(Value::as_str) == Some("input_json_delta") {
                if let Some(fragment) = delta.get("partial_json").and_then(Value::as_str) {
                    input_json.push_str(fragment);
                }
            }
        }
        None => {}
    }
    Ok(())
}

fn translate_content_block_stop(
    state: &mut ClaudeTranslationState,
    event: &Map<String, Value>,
    out: &mut TranslateOutput,
) -> Result<()> {
    let index = event
        .get("index")
        .and_then(Value::as_u64)
        .context("missing Claude content block stop index")? as usize;
    if let Some(block) = state.current_blocks.remove(&index) {
        match block {
            ClaudeBlockState::Text { item_id, text } => {
                out.lines.push(
                    json!({
                        "method": "item/completed",
                        "params": {
                            "item": {
                                "id": item_id,
                                "type": "agentMessage",
                                "text": text,
                            }
                        }
                    })
                    .to_string(),
                );
            }
            ClaudeBlockState::ToolUse {
                item_id,
                name,
                input_json,
            } => {
                let input = parse_partial_json_object(&input_json);
                let hidden = should_hide_claude_tool_transcript(&name, &Value::Object(input.clone()));
                state.tool_calls.insert(
                    item_id.clone(),
                    ClaudeToolCall {
                        name: name.clone(),
                        input: Value::Object(input.clone()),
                    },
                );
                if hidden {
                    return Ok(());
                }
                out.lines.push(
                    json!({
                        "method": "item/completed",
                        "params": {
                            "item": {
                                "id": item_id,
                                "type": "toolCall",
                                "tool": name,
                                "name": name,
                                "input": input,
                            }
                        }
                    })
                    .to_string(),
                );
            }
        }
    }
    Ok(())
}

fn translate_assistant_record(
    state: &mut ClaudeTranslationState,
    root: &Map<String, Value>,
    out: &mut TranslateOutput,
) {
    if let Some(session_id) = root
        .get("session_id")
        .or_else(|| root.get("sessionId"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        state.session_id = Some(session_id.to_string());
    }
    if let Some(model) = root
        .get("message")
        .and_then(Value::as_object)
        .and_then(|message| message.get("model"))
        .and_then(Value::as_str)
        .map(normalize_claude_model_name)
    {
        state.model = Some(model);
    }
    synthesize_assistant_snapshot(state, root, out);
}

fn translate_user_record(
    state: &mut ClaudeTranslationState,
    root: &Map<String, Value>,
    out: &mut TranslateOutput,
) {
    let message = root
        .get("message")
        .and_then(Value::as_object)
        .filter(|msg| msg.get("role").and_then(Value::as_str) == Some("user"));
    let Some(message) = message else { return };
    let Some(content) = message.get("content").and_then(Value::as_array) else {
        return;
    };
    for part in content {
        if part.get("type").and_then(Value::as_str) != Some("tool_result") {
            continue;
        }
        let tool_use_id = part
            .get("tool_use_id")
            .and_then(Value::as_str)
            .unwrap_or("");
        let Some(tool_call) = state.tool_calls.remove(tool_use_id) else {
            continue;
        };
        let hide_transcript = should_hide_claude_tool_transcript(&tool_call.name, &tool_call.input);
        let pending_approval =
            claude_exit_plan_approval_from_tool_call(&tool_call, tool_use_id, part);
        if !hide_transcript {
            if let Some(line) = synthetic_tool_result_line(
                &tool_call,
                tool_use_id,
                part,
                root.get("tool_use_result"),
            ) {
                out.lines.push(line);
            }
        }
        if let Some(approval) = pending_approval {
            out.lines.push(claude_exit_plan_request_line(&approval));
        }
    }
}

fn translate_result_record(
    state: &mut ClaudeTranslationState,
    root: &Map<String, Value>,
    out: &mut TranslateOutput,
) {
    if let Some(usage) = root.get("usage").and_then(Value::as_object) {
        out.lines
            .push(synthetic_token_usage_line(usage, state.model.as_deref()));
    }
    if let Some(turn_id) = state.current_turn_id.take() {
        let status = if state.interrupt_requested
            || root.get("terminal_reason").and_then(Value::as_str) == Some("interrupted")
        {
            state.interrupt_requested = false;
            "interrupted"
        } else {
            "completed"
        };
        out.lines.push(
            json!({
                "method": "turn/completed",
                "params": {
                    "turn": {
                        "id": turn_id,
                        "status": status,
                    }
                }
            })
            .to_string(),
        );
        state.current_blocks.clear();
    }
}
