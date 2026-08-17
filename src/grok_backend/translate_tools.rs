//! Grok ACP tool-call and usage translation.

use serde_json::{json, Map, Value};

use super::super::types::{
    jsonrpc_notification, should_hide_tool_result, token_usage_line, tool_result_text,
    GrokContextUsage, GrokToolCall, GrokTranslationState, TranslateOutput,
    GROK_DEFAULT_CONTEXT_WINDOW,
};
use super::{ensure_turn_started, flush_open_items};

pub(super) fn translate_tool_call(
    state: &mut GrokTranslationState,
    update: &Map<String, Value>,
    out: &mut TranslateOutput,
) {
    let Some(tool_id) = update
        .get("toolCallId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
    else {
        return;
    };
    flush_open_items(state, out);
    ensure_turn_started(state, out);
    let name = tool_display_name(update);
    let kind = update
        .get("kind")
        .and_then(Value::as_str)
        .map(str::to_string);
    let input = update.get("rawInput").cloned().unwrap_or(Value::Null);
    let hidden_result = should_hide_tool_result(&name, kind.as_deref());
    let tool = GrokToolCall {
        name: name.clone(),
        kind,
        input: input.clone(),
        hidden_result,
    };
    if !hidden_result || !is_internal_tool(&name) {
        out.lines.push(jsonrpc_notification(
            "item/completed",
            json!({
                "item": {
                    "id": tool_id,
                    "type": "toolCall",
                    "tool": name,
                    "name": name,
                    "input": input,
                }
            }),
        ));
    }
    state.tools.insert(tool_id.to_string(), tool);
}

pub(super) fn translate_tool_update(
    state: &mut GrokTranslationState,
    update: &Map<String, Value>,
    out: &mut TranslateOutput,
) {
    let Some(tool_id) = update
        .get("toolCallId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
    else {
        return;
    };
    if let Some(kind) = update.get("kind").and_then(Value::as_str) {
        if let Some(tool) = state.tools.get_mut(tool_id) {
            tool.kind = Some(kind.to_string());
            tool.hidden_result = should_hide_tool_result(&tool.name, Some(kind));
        }
    }
    let status = update.get("status").and_then(Value::as_str);
    if !matches!(status, Some("completed" | "failed")) {
        return;
    }
    let hidden = state
        .tools
        .get(tool_id)
        .map(|tool| tool.hidden_result)
        .unwrap_or(false);
    if hidden {
        return;
    }
    let text = tool_result_text(update).unwrap_or_default();
    out.lines.push(jsonrpc_notification(
        "item/completed",
        json!({
            "item": {
                "id": format!("{tool_id}-result"),
                "type": "toolResult",
                "output": text,
                "text": text,
            }
        }),
    ));
}

pub(super) fn translate_usage_update(
    state: &mut GrokTranslationState,
    update: &Map<String, Value>,
    out: &mut TranslateOutput,
) {
    let Some(used) = update.get("used").and_then(Value::as_u64) else {
        return;
    };
    let max = update
        .get("size")
        .and_then(Value::as_u64)
        .filter(|max| *max > 0)
        .unwrap_or(GROK_DEFAULT_CONTEXT_WINDOW);
    let usage = GrokContextUsage {
        used: used.min(max),
        max,
    };
    state.last_usage = Some(usage);
    if !state.replaying {
        out.lines.push(token_usage_line(usage));
    }
}

pub(super) fn parse_usage(usage: Option<&Value>) -> Option<GrokContextUsage> {
    let usage = usage?.as_object()?;
    let used = usage
        .get("totalTokens")
        .or_else(|| usage.get("total_tokens"))
        .and_then(Value::as_u64)?;
    let max = usage
        .get("contextWindow")
        .or_else(|| usage.get("modelContextWindow"))
        .and_then(Value::as_u64)
        .filter(|max| *max > 0)
        .unwrap_or(GROK_DEFAULT_CONTEXT_WINDOW);
    Some(GrokContextUsage {
        used: used.min(max),
        max,
    })
}

fn tool_display_name(update: &Map<String, Value>) -> String {
    update
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .or_else(|| {
            update
                .get("toolName")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|name| !name.is_empty())
        })
        .unwrap_or("Tool")
        .to_string()
}

fn is_internal_tool(name: &str) -> bool {
    matches!(
        name.trim().to_ascii_lowercase().as_str(),
        "todo_write" | "todowrite" | "todo_read"
    )
}
