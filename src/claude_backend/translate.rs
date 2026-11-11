//! Claude CLI stream event translation into the codex notification protocol.

use anyhow::{Context, Result};
use serde_json::{json, Map, Value};

use super::snapshot::synthesize_assistant_snapshot;
use super::thinking::{start_thinking_block, thinking_completed, thinking_delta};
use super::types::{
    begin_claude_message, claude_message_item_id, claude_tool_item_id, ensure_claude_turn_started,
    parse_partial_json_object, should_hide_claude_tool_transcript, ClaudeBlockState,
    ClaudeToolCall, ClaudeTranslationState, TranslateOutput,
};
use super::user_record::translate_user_record;

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
    state.session_id = Some(session_id.to_string());
    out.lines.push(
        json!({
            "method": "thread/initialized",
            "params": { "thread": { "id": session_id } },
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
            let message_id = event
                .get("message")
                .and_then(Value::as_object)
                .and_then(|message| message.get("id"))
                .and_then(Value::as_str);
            begin_claude_message(state, message_id);
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
    if let Some(kind) = block.get("type").and_then(Value::as_str) {
        state.streamed_blocks.insert(index, kind.to_string());
    }
    match block.get("type").and_then(Value::as_str) {
        Some("thinking") => start_thinking_block(state, index, block, out),
        Some("text") => start_text_block(state, index, block, out),
        Some("tool_use") => start_tool_use_block(state, index, block),
        _ => {}
    }
    Ok(())
}

fn start_text_block(
    state: &mut ClaudeTranslationState,
    index: usize,
    block: &Map<String, Value>,
    out: &mut TranslateOutput,
) {
    state.current_message_has_content_blocks = true;
    let item_id = claude_message_item_id(state, index);
    state.current_blocks.insert(
        index,
        ClaudeBlockState::Text {
            item_id: item_id.clone(),
            text: block
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
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
    if let Some(text) = block
        .get("text")
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
    {
        out.lines.push(
            json!({"method":"item/agentMessage/delta","params":{"itemId":item_id,"delta":text}})
                .to_string(),
        );
    }
}

fn start_tool_use_block(
    state: &mut ClaudeTranslationState,
    index: usize,
    block: &Map<String, Value>,
) {
    state.current_message_has_content_blocks = true;
    let item_id = block
        .get("id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| claude_tool_item_id(state, index));
    let name = block
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("Tool")
        .to_string();
    state.current_blocks.insert(
        index,
        ClaudeBlockState::ToolUse {
            item_id,
            name,
            input_json: String::new(),
        },
    );
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
        Some(ClaudeBlockState::Thinking { item_id, text }) => {
            thinking_delta(item_id, text, delta, out)
        }
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
        Some(ClaudeBlockState::ToolUse { input_json, .. })
            if delta.get("type").and_then(Value::as_str) == Some("input_json_delta") =>
        {
            if let Some(fragment) = delta.get("partial_json").and_then(Value::as_str) {
                input_json.push_str(fragment);
            }
        }
        Some(ClaudeBlockState::ToolUse { .. }) | None => {}
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
    let Some(block) = state.current_blocks.remove(&index) else {
        return Ok(());
    };
    match block {
        ClaudeBlockState::Thinking { item_id, text } => thinking_completed(item_id, text, out),
        ClaudeBlockState::Text { item_id, text } => emit_text_block_completed(item_id, text, out),
        ClaudeBlockState::ToolUse {
            item_id,
            name,
            input_json,
        } => emit_tool_use_block_completed(state, item_id, name, input_json, out),
    }
    Ok(())
}

fn emit_text_block_completed(item_id: String, text: String, out: &mut TranslateOutput) {
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

fn emit_tool_use_block_completed(
    state: &mut ClaudeTranslationState,
    item_id: String,
    name: String,
    input_json: String,
    out: &mut TranslateOutput,
) {
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
        return;
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
    synthesize_assistant_snapshot(state, root, out);
}

fn translate_result_record(
    state: &mut ClaudeTranslationState,
    root: &Map<String, Value>,
    out: &mut TranslateOutput,
) {
    if let Some(turn_id) = state.current_turn_id.take() {
        let status = if state.interrupt_requested
            || matches!(
                root.get("terminal_reason").and_then(Value::as_str),
                Some("aborted_streaming" | "aborted_tools")
            ) {
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
