//! Claude user record translation for tool-result follow-ups.

use serde_json::{Map, Value};

use super::tool_results::{fallback_tool_result_item, synthetic_tool_result_line};
use super::types::{should_hide_claude_tool_transcript, ClaudeTranslationState, TranslateOutput};

// --- User Records ---

pub(super) fn translate_user_record(
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
            if let Some(item) = fallback_tool_result_item(tool_use_id, part) {
                out.lines.push(
                    serde_json::json!({"method":"item/completed","params":{"item":item}})
                        .to_string(),
                );
            }
            continue;
        };
        let hide_transcript = should_hide_claude_tool_transcript(&tool_call.name, &tool_call.input);
        if !hide_transcript {
            if let Some(line) = synthetic_tool_result_line(
                &tool_call,
                tool_use_id,
                part,
                root.get("tool_use_result")
                    .or_else(|| root.get("toolUseResult")),
            ) {
                out.lines.push(line);
            }
        }
    }
}
