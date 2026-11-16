//! Preserve complete text chunks across native message ids, roles and tool boundaries.

use std::collections::HashSet;

use serde_json::{json, Value};

use crate::grok_backend::translate::{finish_prompt_turn, translate_grok_notification};
use crate::grok_backend::types::GrokTranslationState;

fn text(kind: &str, text: &str, id: Option<&str>) -> Value {
    json!({"sessionUpdate":kind,"messageId":id,"content":{"type":"text","text":text}})
}

fn records(replaying: bool, updates: Vec<Value>) -> (Vec<Value>, String) {
    let mut state = GrokTranslationState {
        replaying,
        ..Default::default()
    };
    let mut lines = Vec::new();
    for update in updates {
        lines.extend(
            translate_grok_notification(&mut state, "session/update", &json!({"update":update}))
                .lines,
        );
    }
    lines.extend(finish_prompt_turn(&mut state, false).lines);
    let items = lines
        .iter()
        .filter_map(|line| {
            let row: Value = serde_json::from_str(line).expect("normalized event");
            let item = &row["params"]["item"];
            ((row["method"] == "item/started" && item["type"] == "userMessage")
                || (row["method"] == "item/completed"
                    && matches!(item["type"].as_str(), Some("agentMessage" | "reasoning"))))
            .then(|| item.clone())
        })
        .collect();
    (items, lines.join("\n"))
}

fn texts(items: &[Value]) -> Vec<&str> {
    items
        .iter()
        .map(|item| {
            item["text"]
                .as_str()
                .or_else(|| item["content"][0]["text"].as_str())
                .expect("exact native text")
        })
        .collect()
}

#[test]
fn replay_keeps_native_message_boundaries_and_joins_only_chunks_of_the_same_message() {
    let (items, lines) = records(
        true,
        vec![
            text("user_message_chunk", "first", Some("PRIVATE_NATIVE_USER_A")),
            text("user_message_chunk", " tail", Some("PRIVATE_NATIVE_USER_A")),
            text(
                "agent_message_chunk",
                "one",
                Some("PRIVATE_NATIVE_ANSWER_A"),
            ),
            text(
                "agent_message_chunk",
                "two",
                Some("PRIVATE_NATIVE_ANSWER_B"),
            ),
            text(
                "user_message_chunk",
                "second",
                Some("PRIVATE_NATIVE_USER_B"),
            ),
            text("agent_message_chunk", "last", None),
        ],
    );
    assert_eq!(
        texts(&items),
        ["first tail", "one", "two", "second", "last"]
    );
    assert_eq!(
        items
            .iter()
            .map(|item| item["id"].as_str().expect("row id"))
            .collect::<HashSet<_>>()
            .len(),
        items.len()
    );
    assert!(!lines.contains("PRIVATE_NATIVE"));
    assert!(!lines.contains("turn/started"));
    assert!(!lines.contains("turn/completed"));
}

#[test]
fn live_and_replayed_text_keep_distinct_rows_on_both_sides_of_a_tool() {
    for replaying in [false, true] {
        let (items, _) = records(
            replaying,
            vec![
                text("user_message_chunk", "human", None),
                text("agent_message_chunk", "before", None),
                json!({"sessionUpdate":"tool_call","toolCallId":"tool","title":"Command","kind":"execute","rawInput":{}}),
                text("agent_message_chunk", "after", None),
                text("agent_message_chunk", " tail", None),
                text("agent_thought_chunk", "thought", None),
                text("agent_message_chunk", "final", None),
            ],
        );
        assert_eq!(
            texts(&items),
            ["human", "before", "after tail", "thought", "final"]
        );
        assert_eq!(
            items
                .iter()
                .map(|item| item["id"].as_str().expect("row id"))
                .collect::<HashSet<_>>()
                .len(),
            items.len()
        );
    }
}
