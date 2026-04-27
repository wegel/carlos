//! Claude user record translation for tool-result follow-ups.

use serde_json::{Map, Value};

use super::exit_plan::{
    claude_exit_plan_approval_from_tool_call, claude_exit_plan_request_line,
    synthetic_tool_result_line,
};
use super::types::{
    should_hide_claude_tool_transcript, ClaudeTranslationState, TranslateOutput,
};

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
