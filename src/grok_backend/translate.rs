//! Grok ACP `session/update` translation into the Codex notification protocol.

#[path = "translate_tools.rs"]
mod translate_tools;

use serde_json::{json, Map, Value};

use self::translate_tools::{
    parse_usage, translate_tool_call, translate_tool_update, translate_usage_update,
};
use super::types::{
    content_text, jsonrpc_notification, token_usage_line, GrokTranslationState, TranslateOutput,
};

// --- Public entry ---

pub(super) fn translate_grok_notification(
    state: &mut GrokTranslationState,
    method: &str,
    params: &Value,
) -> TranslateOutput {
    let mut out = TranslateOutput::default();
    match method {
        "session/update" => translate_session_update(state, params, &mut out),
        "_x.ai/session/update" => translate_xai_session_update(state, params, &mut out),
        "_x.ai/session_notification" => translate_session_notification(state, params, &mut out),
        _ => {}
    }
    out
}

pub(crate) fn finish_prompt_turn(
    state: &mut GrokTranslationState,
    interrupted: bool,
) -> TranslateOutput {
    let mut out = TranslateOutput::default();
    flush_open_items(state, &mut out);
    if !state.replaying {
        emit_turn_completed(state, interrupted, &mut out);
    }
    state.interrupt_requested = false;
    out
}

pub(crate) fn flush_replay_tail(state: &mut GrokTranslationState) -> TranslateOutput {
    let mut out = TranslateOutput::default();
    flush_open_items(state, &mut out);
    out
}

// --- session/update ---

fn translate_session_update(
    state: &mut GrokTranslationState,
    params: &Value,
    out: &mut TranslateOutput,
) {
    remember_session_id(state, params);
    let Some(update) = params.get("update").and_then(Value::as_object) else {
        return;
    };
    match update.get("sessionUpdate").and_then(Value::as_str) {
        Some("user_message_chunk") => translate_user_chunk(state, update, out),
        Some("agent_message_chunk") => translate_agent_chunk(state, update, out),
        Some("agent_thought_chunk") => translate_thought_chunk(state, update, out),
        Some("tool_call") => translate_tool_call(state, update, out),
        Some("tool_call_update") => translate_tool_update(state, update, out),
        Some("usage_update") => translate_usage_update(state, update, out),
        _ => {}
    }
}

fn translate_xai_session_update(
    state: &mut GrokTranslationState,
    params: &Value,
    out: &mut TranslateOutput,
) {
    remember_session_id(state, params);
    let Some(update) = params.get("update").and_then(Value::as_object) else {
        return;
    };
    if update.get("sessionUpdate").and_then(Value::as_str) != Some("turn_completed") {
        return;
    }
    if let Some(usage) = parse_usage(update.get("usage")) {
        state.last_usage = Some(usage);
        if !state.replaying {
            out.lines.push(token_usage_line(usage));
        }
    }
    if !state.replaying {
        let interrupted = update.get("stop_reason").and_then(Value::as_str) == Some("cancelled")
            || state.interrupt_requested;
        flush_open_items(state, out);
        emit_turn_completed(state, interrupted, out);
    }
}

fn translate_session_notification(
    state: &mut GrokTranslationState,
    params: &Value,
    out: &mut TranslateOutput,
) {
    remember_session_id(state, params);
    let Some(update) = params.get("update").and_then(Value::as_object) else {
        return;
    };
    if update.get("sessionUpdate").and_then(Value::as_str) != Some("model_changed") {
        return;
    }
    if let Some(model) = update
        .get("model_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|model| !model.is_empty())
    {
        state.model = Some(model.to_string());
    }
    if let Some(effort) = update
        .get("reasoning_effort")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|effort| !effort.is_empty())
    {
        state.effort = Some(effort.to_string());
    }
    let Some(session_id) = state.session_id.as_deref() else {
        return;
    };
    out.lines.push(jsonrpc_notification(
        "thread/initialized",
        json!({
            "thread": { "id": session_id },
            "model": state.model,
            "effort": state.effort,
        }),
    ));
}

// --- Chunks ---

fn translate_user_chunk(
    state: &mut GrokTranslationState,
    update: &Map<String, Value>,
    _out: &mut TranslateOutput,
) {
    if !state.replaying {
        return;
    }
    let Some(text) = content_text(update.get("content").unwrap_or(&Value::Null)) else {
        return;
    };
    state.current_user_text.push_str(&text);
}

fn translate_agent_chunk(
    state: &mut GrokTranslationState,
    update: &Map<String, Value>,
    out: &mut TranslateOutput,
) {
    let Some(text) = content_text(update.get("content").unwrap_or(&Value::Null)) else {
        return;
    };
    flush_user_text(state, out);
    flush_thought(state, out);
    ensure_turn_started(state, out);
    let item_id = ensure_message_item(state, out);
    state.current_message_text.push_str(&text);
    out.lines.push(jsonrpc_notification(
        "item/agentMessage/delta",
        json!({ "itemId": item_id, "delta": text }),
    ));
}

fn translate_thought_chunk(
    state: &mut GrokTranslationState,
    update: &Map<String, Value>,
    out: &mut TranslateOutput,
) {
    let Some(text) = content_text(update.get("content").unwrap_or(&Value::Null)) else {
        return;
    };
    flush_user_text(state, out);
    flush_message(state, out);
    ensure_turn_started(state, out);
    let item_id = ensure_thought_item(state, out);
    state.current_thought_text.push_str(&text);
    out.lines.push(jsonrpc_notification(
        "item/reasoning/summaryTextDelta",
        json!({ "itemId": item_id, "delta": text }),
    ));
}

// --- Flush / emit ---

pub(super) fn flush_open_items(state: &mut GrokTranslationState, out: &mut TranslateOutput) {
    flush_user_text(state, out);
    flush_thought(state, out);
    flush_message(state, out);
}

fn flush_user_text(state: &mut GrokTranslationState, out: &mut TranslateOutput) {
    if state.current_user_text.trim().is_empty() {
        state.current_user_text.clear();
        return;
    }
    let text = std::mem::take(&mut state.current_user_text);
    out.lines.push(jsonrpc_notification(
        "item/started",
        json!({
            "item": {
                "id": format!("grok-user-{}", state.next_turn_seq),
                "type": "userMessage",
                "content": [{ "type": "text", "text": text }],
            }
        }),
    ));
}

fn flush_message(state: &mut GrokTranslationState, out: &mut TranslateOutput) {
    let Some(item_id) = state.current_message_id.take() else {
        return;
    };
    let text = std::mem::take(&mut state.current_message_text);
    out.lines.push(jsonrpc_notification(
        "item/completed",
        json!({
            "item": {
                "id": item_id,
                "type": "agentMessage",
                "text": text,
            }
        }),
    ));
}

fn flush_thought(state: &mut GrokTranslationState, out: &mut TranslateOutput) {
    let Some(item_id) = state.current_thought_id.take() else {
        return;
    };
    let text = std::mem::take(&mut state.current_thought_text);
    out.lines.push(jsonrpc_notification(
        "item/completed",
        json!({
            "item": {
                "id": item_id,
                "type": "reasoning",
                "text": text,
            }
        }),
    ));
}

pub(super) fn ensure_turn_started(state: &mut GrokTranslationState, out: &mut TranslateOutput) {
    if state.replaying || state.current_turn_id.is_some() {
        return;
    }
    state.next_turn_seq = state.next_turn_seq.saturating_add(1);
    let turn_id = format!("grok-turn-{}", state.next_turn_seq);
    state.current_turn_id = Some(turn_id.clone());
    out.lines.push(jsonrpc_notification(
        "turn/started",
        json!({ "turn": { "id": turn_id } }),
    ));
}

fn emit_turn_completed(
    state: &mut GrokTranslationState,
    interrupted: bool,
    out: &mut TranslateOutput,
) {
    let turn_id = state
        .current_turn_id
        .take()
        .unwrap_or_else(|| "grok-turn-pending".to_string());
    let status = if interrupted {
        "interrupted"
    } else {
        "completed"
    };
    out.lines.push(jsonrpc_notification(
        "turn/completed",
        json!({ "turn": { "id": turn_id, "status": status } }),
    ));
}

fn ensure_message_item(state: &mut GrokTranslationState, out: &mut TranslateOutput) -> String {
    if let Some(item_id) = state.current_message_id.clone() {
        return item_id;
    }
    let item_id = format!("grok-msg-{}", state.next_turn_seq);
    state.current_message_id = Some(item_id.clone());
    out.lines.push(jsonrpc_notification(
        "item/started",
        json!({ "item": { "id": item_id, "type": "agentMessage" } }),
    ));
    item_id
}

fn ensure_thought_item(state: &mut GrokTranslationState, out: &mut TranslateOutput) -> String {
    if let Some(item_id) = state.current_thought_id.clone() {
        return item_id;
    }
    let item_id = format!("grok-thought-{}", state.next_turn_seq);
    state.current_thought_id = Some(item_id.clone());
    out.lines.push(jsonrpc_notification(
        "item/started",
        json!({ "item": { "id": item_id, "type": "reasoning" } }),
    ));
    item_id
}

fn remember_session_id(state: &mut GrokTranslationState, params: &Value) {
    if let Some(session_id) = params
        .get("sessionId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        state.session_id = Some(session_id.to_string());
    }
}

#[cfg(test)]
pub(crate) fn translate_grok_notification_for_test(
    state: &mut GrokTranslationState,
    method: &str,
    params: Value,
) -> TranslateOutput {
    translate_grok_notification(state, method, &params)
}
