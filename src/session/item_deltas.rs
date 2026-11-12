//! Apply accepted common item deltas under their native turn and item identity.

use serde_json::{Map, Value};

use super::item_changes::ItemChanges;
use super::item_receive::receive_reasoning;
use super::state::SessionState;

#[derive(Clone, Copy)]
enum Delta {
    Agent(bool),
    Reasoning,
    Command,
}

/// Recognize the existing common delta methods without guessing unknown tool variants.
pub(super) fn receive(
    session: &mut SessionState,
    changes: &mut ItemChanges,
    method: &str,
    params: &Map<String, Value>,
) -> bool {
    let kind = match method {
        "item/agentMessage/delta" => Delta::Agent(true),
        "item/reasoning/summaryTextDelta" => Delta::Reasoning,
        "item/commandExecution/outputDelta" => Delta::Command,
        "item/toolCall/delta"
        | "item/tool_call/delta"
        | "item/toolInvocation/delta"
        | "item/functionCall/delta"
        | "item/mcpToolCall/delta"
        | "item/toolResult/delta"
        | "item/toolOutput/delta"
        | "item/tool_result/delta"
        | "item/functionCallOutput/delta"
        | "item/mcpToolResult/delta" => {
            if method.contains("Result/")
                || method.contains("Output/")
                || method.contains("tool_result/")
            {
                Delta::Command
            } else {
                Delta::Agent(false)
            }
        }
        _ => return false,
    };
    if let (Some(item), Some(delta)) = (
        params.get("itemId").and_then(Value::as_str),
        params.get("delta").and_then(Value::as_str),
    ) {
        apply(session, changes, kind, item, delta);
    }
    true
}

fn apply(
    session: &mut SessionState,
    changes: &mut ItemChanges,
    kind: Delta,
    item: &str,
    delta: &str,
) {
    match kind {
        Delta::Agent(blocked) => {
            changes.push(session.upsert_agent_delta(&session.backend_entry_key(item), delta));
            if blocked {
                changes.push(session.maybe_disable_ralph_on_blocked_marker());
            }
        }
        Delta::Reasoning => receive_reasoning(session, changes, item, delta),
        Delta::Command => changes
            .push(session.upsert_command_output_delta(&session.backend_entry_key(item), delta)),
    }
}
