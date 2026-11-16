//! Grok ACP `session/update` translation into the Codex notification protocol.

#[path = "translate_text.rs"]
mod text;
#[path = "translate_tools.rs"]
mod translate_tools;

use serde_json::{json, Value};

use self::text::{translate_agent_chunk, translate_thought_chunk, translate_user_chunk};
use self::translate_tools::{
    parse_usage, translate_tool_call, translate_tool_update, translate_usage_update,
};
use super::types::{jsonrpc_notification, token_usage_line, GrokTranslationState, TranslateOutput};

pub(super) use self::text::flush_open_items;

// --- Public entry ---

pub(super) fn translate_grok_notification(
    state: &mut GrokTranslationState,
    method: &str,
    params: &Value,
) -> TranslateOutput {
    let mut out = TranslateOutput::default();
    if state.publish_identity {
        if let Some(id) = params["sessionId"].as_str().filter(|id| !id.is_empty()) {
            out.lines.push(jsonrpc_notification(
                "thread/initialized",
                json!({"thread":{"id":id}}),
            ));
            state.publish_identity = false;
        }
    }
    match method {
        "session/update" => translate_session_update(state, params, &mut out),
        "_x.ai/session/update" => translate_xai_session_update(state, params, &mut out),
        "_x.ai/session_notification" => translate_session_notification(state, params, &mut out),
        _ => {}
    }
    out
}

pub(super) fn mark_prompt_started(state: &mut GrokTranslationState) {
    state.prompt_epoch = state
        .prompt_epoch
        .checked_add(1)
        .expect("Grok prompt counter exhausted");
    state.suppress_user_echo = true;
    state.ignore_stale_updates = false;
    state.turn_completed_emitted = false;
}

pub(super) fn interrupt_prompt_turn(state: &mut GrokTranslationState) -> TranslateOutput {
    state.interrupt_requested = true;
    state.ignore_stale_updates = true;
    state.suppress_user_echo = false;
    finish_prompt_turn(state, true)
}

/// Finish the current Grok prompt and emit its remaining item and turn messages.
pub fn finish_prompt_turn(state: &mut GrokTranslationState, interrupted: bool) -> TranslateOutput {
    let mut out = TranslateOutput::default();
    flush_open_items(state, &mut out);
    if !state.replaying {
        emit_turn_completed(state, interrupted, &mut out);
    }
    if interrupted {
        state.ignore_stale_updates = true;
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
    let kind = update.get("sessionUpdate").and_then(Value::as_str);
    if should_drop_stale(state, kind) {
        return;
    }
    match kind {
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

pub(super) fn ensure_turn_started(state: &mut GrokTranslationState, out: &mut TranslateOutput) {
    if state.replaying || state.current_turn_id.is_some() || state.ignore_stale_updates {
        return;
    }
    start_turn(state, out);
}

fn start_turn(state: &mut GrokTranslationState, out: &mut TranslateOutput) {
    state.next_turn_seq = state.next_turn_seq.saturating_add(1);
    let turn_id = format!("grok-turn-{}", state.next_turn_seq);
    state.current_turn_id = Some(turn_id.clone());
    state.turn_completed_emitted = false;
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
    if state.turn_completed_emitted && state.current_turn_id.is_none() {
        return;
    }
    if state.current_turn_id.is_none() && state.prompt_epoch > 0 {
        start_turn(state, out);
    }
    let turn_id = state
        .current_turn_id
        .take()
        .unwrap_or_else(|| "grok-turn-pending".to_string());
    state.turn_completed_emitted = true;
    state.suppress_user_echo = false;
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

fn should_drop_stale(state: &GrokTranslationState, kind: Option<&str>) -> bool {
    state.ignore_stale_updates && !matches!(kind, Some("user_message_chunk" | "usage_update"))
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

#[cfg(any(test, feature = "test-support"))]
/// Translate one native Grok notice through the ordinary fixture translator.
pub fn translate_grok_notification_for_test(
    state: &mut GrokTranslationState,
    method: &str,
    params: Value,
) -> TranslateOutput {
    translate_grok_notification(state, method, &params)
}

#[cfg(test)]
#[path = "translate_tests.rs"]
mod tests;
