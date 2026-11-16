//! Keep native text chunks in distinct user, answer and thinking rows during live work and replay.

use serde_json::{json, Map, Value};

use super::ensure_turn_started;
use crate::grok_backend::types::{
    content_text, jsonrpc_notification, GrokTextCursor, GrokTranslationState, TranslateOutput,
};

pub(super) fn translate_user_chunk(
    state: &mut GrokTranslationState,
    update: &Map<String, Value>,
    out: &mut TranslateOutput,
) {
    let Some(text) = content_text(update.get("content").unwrap_or(&Value::Null)) else {
        return;
    };
    if !state.replaying && state.suppress_user_echo {
        return;
    }
    prepare_text(state, update, "user", out);
    if state.replaying {
        state.current_user_text.push_str(&text);
        return;
    }
    state.ignore_stale_updates = false;
    flush_thought(state, out);
    flush_message(state, out);
    ensure_turn_started(state, out);
    state.current_user_text.push_str(&text);
    flush_user_text(state, out);
}

pub(super) fn translate_agent_chunk(
    state: &mut GrokTranslationState,
    update: &Map<String, Value>,
    out: &mut TranslateOutput,
) {
    let Some(text) = content_text(update.get("content").unwrap_or(&Value::Null)) else {
        return;
    };
    prepare_text(state, update, "agent", out);
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

pub(super) fn translate_thought_chunk(
    state: &mut GrokTranslationState,
    update: &Map<String, Value>,
    out: &mut TranslateOutput,
) {
    let Some(text) = content_text(update.get("content").unwrap_or(&Value::Null)) else {
        return;
    };
    prepare_text(state, update, "thought", out);
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

pub(in crate::grok_backend) fn flush_open_items(
    state: &mut GrokTranslationState,
    out: &mut TranslateOutput,
) {
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
                "id": state.next_text_id("user"),
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

fn ensure_message_item(state: &mut GrokTranslationState, out: &mut TranslateOutput) -> String {
    if let Some(item_id) = state.current_message_id.clone() {
        return item_id;
    }
    let item_id = state.next_text_id("msg");
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
    let item_id = state.next_text_id("thought");
    state.current_thought_id = Some(item_id.clone());
    out.lines.push(jsonrpc_notification(
        "item/started",
        json!({ "item": { "id": item_id, "type": "reasoning" } }),
    ));
    item_id
}

fn prepare_text(
    state: &mut GrokTranslationState,
    update: &Map<String, Value>,
    kind: &'static str,
    out: &mut TranslateOutput,
) {
    let native = update
        .get("messageId")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty());
    if state
        .current_text
        .as_ref()
        .is_some_and(|current| current.kind == kind && current.native.as_deref() == native)
    {
        return;
    }
    flush_open_items(state, out);
    state.current_text = Some(GrokTextCursor {
        kind,
        native: native.map(str::to_owned),
    });
}

#[cfg(test)]
#[path = "translate_text_tests.rs"]
mod tests;
