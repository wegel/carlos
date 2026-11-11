//! Maps visible native thinking into reasoning rows without exposing redacted data.

use serde_json::{json, Map, Value};

use super::types::{
    claude_message_item_id, ClaudeBlockState, ClaudeTranslationState, TranslateOutput,
};

/// Start one visible thinking block in the existing reasoning renderer.
pub(super) fn start_thinking_block(
    state: &mut ClaudeTranslationState,
    index: usize,
    block: &Map<String, Value>,
    output: &mut TranslateOutput,
) {
    let id = claude_message_item_id(state, index);
    let text = block
        .get("thinking")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    output.lines.push(
        json!({"method":"item/started","params":{"item":{"id":id,"type":"reasoning"}}}).to_string(),
    );
    if !text.is_empty() {
        output.lines.push(
            json!({"method":"item/reasoning/summaryTextDelta","params":{"itemId":id,"delta":text}})
                .to_string(),
        );
    }
    state.current_message_has_content_blocks = true;
    state
        .current_blocks
        .insert(index, ClaudeBlockState::Thinking { item_id: id, text });
}

/// Forward visible thinking deltas; signatures and estimated token counts are not text.
pub(super) fn thinking_delta(
    id: &str,
    text: &mut String,
    delta: &Map<String, Value>,
    output: &mut TranslateOutput,
) {
    if delta.get("type").and_then(Value::as_str) != Some("thinking_delta") {
        return;
    }
    let Some(fragment) = delta
        .get("thinking")
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
    else {
        return;
    };
    text.push_str(fragment);
    output.lines.push(
        json!({"method":"item/reasoning/summaryTextDelta","params":{"itemId":id,"delta":fragment}})
            .to_string(),
    );
}

/// Complete a visible reasoning row without interpreting signed or redacted content.
pub(super) fn thinking_completed(id: String, text: String, output: &mut TranslateOutput) {
    output.lines.push(
        json!({"method":"item/completed","params":{"item":{"id":id,"type":"reasoning",
        "summary":[{"text":text}]}}})
        .to_string(),
    );
}
