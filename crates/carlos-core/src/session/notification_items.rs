//! Item-level notification handling: transcript mutation for tool calls, outputs, and history.

use serde_json::Value;

use super::item_changes::ItemChanges;
use super::item_receive::{receive_summary, record_native_turn, record_prompt};

use super::item_history::{agent_role_from_phase, reasoning_summary_text};
use super::item_receive::upsert_native_activity;
use super::models::{MessageKind, Role};
use super::notification_tools::complete_tool_item;
use super::state::SessionState;
use super::tools::{is_tool_call_type, is_tool_output_type, item_text_from_content};

// --- Item Notification Handling ---

/// Apply one normalized item message and its exact source bindings.
pub(crate) fn handle_item_notification(
    session: &mut SessionState,
    changes: &mut ItemChanges,
    method: &str,
    params: &serde_json::Map<String, Value>,
) -> bool {
    let all_parts = matches!(method, "item/started" | "item/completed");
    if let Some(id) = params
        .get("item")
        .and_then(|item| item["id"].as_str())
        .or_else(|| params.get("itemId").and_then(Value::as_str))
    {
        session.prepare_item_source(
            &session.backend_entry_key(id),
            params.get("sourceKey"),
            all_parts,
        );
    }
    let handled = handle_item(session, changes, method, params);
    if let Some(item) = params.get("item") {
        if let Some(id) = item["id"].as_str() {
            session.attach_item_source(
                &session.backend_entry_key(id),
                params.get("sourceKey"),
                all_parts,
            );
        }
    } else if let Some(id) = params.get("itemId").and_then(Value::as_str) {
        session.attach_item_source(
            &session.backend_entry_key(id),
            params.get("sourceKey"),
            all_parts,
        );
    }
    handled
}

fn handle_item(
    session: &mut SessionState,
    changes: &mut ItemChanges,
    method: &str,
    params: &serde_json::Map<String, Value>,
) -> bool {
    match method {
        "codex/event/raw_response_item" => true,
        "item/started" | "item/completed" => handle_item_boundary(session, changes, method, params),
        _ => super::item_deltas::receive(session, method, params),
    }
}

fn handle_item_boundary(
    session: &mut SessionState,
    changes: &mut ItemChanges,
    method: &str,
    params: &serde_json::Map<String, Value>,
) -> bool {
    let live = params
        .get("item")
        .and_then(|v| v["id"].as_str())
        .is_some_and(|id| !session.is_item_completed(&session.backend_entry_key(id)));
    let handled = if method == "item/started" {
        handle_item_started(session, changes, params)
    } else {
        handle_item_completed(session, changes, params)
    };
    record_user_turn_id(session, params);
    if let Some(item) = params.get("item") {
        receive_summary(session, item, live);
    }
    handled
}

fn record_user_turn_id(session: &mut SessionState, params: &serde_json::Map<String, Value>) {
    let Some(item) = params.get("item") else {
        return;
    };
    if item.get("type").and_then(Value::as_str) != Some("userMessage") {
        return;
    }
    session.bind_native_operation(item);
    if let (Some(item_id), Some(turn_id)) = (
        item.get("id").and_then(Value::as_str),
        params.get("turnId").and_then(Value::as_str),
    ) {
        record_native_turn(session, item_id, turn_id);
    }
}

fn handle_item_started(
    session: &mut SessionState,
    changes: &mut ItemChanges,
    params: &serde_json::Map<String, Value>,
) -> bool {
    let Some(item) = params.get("item").and_then(Value::as_object) else {
        return true;
    };
    let Some(t) = item.get("type").and_then(Value::as_str) else {
        return true;
    };
    if item
        .get("id")
        .and_then(Value::as_str)
        .is_some_and(|item_id| session.is_item_completed(&session.backend_entry_key(item_id)))
    {
        return true;
    }
    if matches!(t, "sleep" | "subAgentActivity" | "collabAgentToolCall")
        && upsert_native_activity(session, &Value::Object(item.clone()))
    {
        return true;
    }
    match t {
        "userMessage" => start_user_message(session, changes, item),
        "agentMessage" => start_placeholder(session, item, agent_role_from_phase(item), false),
        "reasoning" => start_placeholder(session, item, Role::Reasoning, false),
        "commandExecution" => start_placeholder(session, item, Role::ToolCall, true),
        t if is_tool_call_type(t) => start_placeholder(session, item, Role::ToolCall, false),
        t if is_tool_output_type(t) => start_placeholder(session, item, Role::ToolOutput, true),
        _ => {}
    }
    true
}

fn start_placeholder(
    session: &mut SessionState,
    item: &serde_json::Map<String, Value>,
    role: Role,
    describe: bool,
) {
    let Some(id) = item.get("id").and_then(Value::as_str) else {
        return;
    };
    let key = session.backend_entry_key(id);
    session.ensure_item_placeholder(&key, role);
    if describe {
        let index = session.item_message_index(&key).expect("native item row");
        super::item_receive::describe_output(session, index, &Value::Object(item.clone()));
    } else if role == Role::ToolCall {
        let index = session.item_message_index(&key).expect("native item row");
        session.describe_tool_facts(index, &Value::Object(item.clone()));
    }
}

fn start_user_message(
    session: &mut SessionState,
    changes: &mut ItemChanges,
    item: &serde_json::Map<String, Value>,
) {
    let Some(id) = item.get("id").and_then(Value::as_str) else {
        return;
    };
    let Some(text) = item_text_from_content(&Value::Object(item.clone())) else {
        return;
    };
    let inserted = session.upsert_mapped_message(
        &session.backend_entry_key(id),
        Role::User,
        text.clone(),
        MessageKind::Plain,
        None,
    );
    if inserted {
        let idx = session
            .item_message_index(&session.backend_entry_key(id))
            .expect("native item row");
        record_prompt(session, changes, &text, Some(idx));
    }
}

fn handle_item_completed(
    session: &mut SessionState,
    changes: &mut ItemChanges,
    params: &serde_json::Map<String, Value>,
) -> bool {
    let Some(item) = params.get("item").and_then(Value::as_object) else {
        return true;
    };
    let Some(kind) = item.get("type").and_then(Value::as_str) else {
        return true;
    };
    let item_value = Value::Object(item.clone());
    let handled = if upsert_native_activity(session, &item_value) {
        true
    } else {
        match kind {
            "userMessage" => complete_user_message(session, changes, item, &item_value),
            "agentMessage" => complete_agent_message(session, item, &item_value),
            "reasoning" => complete_reasoning(session, item, &item_value),
            "contextCompaction" => complete_context_compaction(session, item),
            _ => complete_tool_item(session, kind, &item_value),
        }
    };
    if let Some(item_id) = item.get("id").and_then(Value::as_str) {
        session.mark_item_completed(&session.backend_entry_key(item_id));
    }
    handled
}

fn complete_user_message(
    session: &mut SessionState,
    changes: &mut ItemChanges,
    item: &serde_json::Map<String, Value>,
    item_value: &Value,
) -> bool {
    let Some(item_id) = item.get("id").and_then(Value::as_str) else {
        return true;
    };
    let text = item_text_from_content(item_value).unwrap_or_default();
    let inserted = session.upsert_mapped_message(
        &session.backend_entry_key(item_id),
        Role::User,
        text.clone(),
        MessageKind::Plain,
        None,
    );
    if inserted && !text.is_empty() {
        let idx = session
            .item_message_index(&session.backend_entry_key(item_id))
            .expect("native item row");
        record_prompt(session, changes, &text, Some(idx));
    }
    true
}

fn complete_context_compaction(
    session: &mut SessionState,
    item: &serde_json::Map<String, Value>,
) -> bool {
    if let Some(item_id) = item.get("id").and_then(Value::as_str) {
        session.upsert_mapped_message(
            &session.backend_entry_key(item_id),
            Role::System,
            "↻ Context compacted".to_string(),
            MessageKind::Plain,
            None,
        );
    }
    true
}

fn complete_agent_message(
    session: &mut SessionState,
    item: &serde_json::Map<String, Value>,
    item_value: &Value,
) -> bool {
    let role = agent_role_from_phase(item);
    let item_id = item.get("id").and_then(Value::as_str);
    let text = item
        .get("text")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| item_text_from_content(item_value));
    if let Some(id) = item_id {
        session.upsert_mapped_message(
            &session.backend_entry_key(id),
            role,
            text.unwrap_or_default(),
            MessageKind::Plain,
            None,
        );
        session.maybe_disable_ralph_on_blocked_marker();
        return true;
    }
    if let Some(text) = text {
        session.append_message(role, text);
        session.maybe_disable_ralph_on_blocked_marker();
    }
    true
}

fn complete_reasoning(
    session: &mut SessionState,
    item: &serde_json::Map<String, Value>,
    item_value: &Value,
) -> bool {
    let item_id = item.get("id").and_then(Value::as_str);
    let text = reasoning_summary_text(item_value);
    if let Some(id) = item_id {
        session.upsert_mapped_message(
            &session.backend_entry_key(id),
            Role::Reasoning,
            text.unwrap_or_default(),
            MessageKind::Plain,
            None,
        );
        return true;
    }
    if let Some(text) = text {
        session.append_message(Role::Reasoning, text);
    }
    true
}
