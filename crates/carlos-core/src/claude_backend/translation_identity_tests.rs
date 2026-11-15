//! Preserve native resume guards and reader-assigned item IDs across helper instances.

use serde_json::{json, Value};

use super::conversation::ConversationGuard;
use super::event_normalizer::EventNormalizer;

#[test]
fn session_misroute_event_surfaces_as_error_with_expected_and_actual_ids() {
    let expected = "11111111-1111-4111-8111-111111111111";
    let actual = "22222222-2222-4222-8222-222222222222";
    let mut guard = ConversationGuard::default();
    guard.expected = Some(expected.to_string());
    let error = guard
        .route(json!({"v":1,"type":"event","event":"sdk","message":{
            "type":"system","subtype":"init","session_id":actual
        }}))
        .expect_err("the actual native resume guard rejects foreign init")
        .to_string();
    assert!(error.contains(expected));
    assert!(error.contains(actual));
    assert!(error.contains("session stopped"));
    assert_eq!(guard.expected.as_deref(), Some(expected));
    assert_eq!(guard.epoch, 0);
}

#[test]
fn live_claude_item_ids_are_unique_across_process_respawns() {
    let first = native_item_id(&mut EventNormalizer::default());
    let second = native_item_id(&mut EventNormalizer::default());
    assert_ne!(first, second);
}

fn native_item_id(reader: &mut EventNormalizer) -> String {
    let events = [
        json!({"type":"message_start","message":{"id":"msg_1","role":"assistant","type":"message","content":[]}}),
        json!({"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}),
        json!({"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"hello"}}),
        json!({"type":"content_block_stop","index":1}),
    ];
    let lines = events
        .into_iter()
        .flat_map(|event| {
            reader
                .route(
                    &json!({"v":1,"type":"event","event":"sdk","message":{
                        "type":"stream_event","event":event
                    }}),
                    None,
                    &Value::Null,
                    0,
                )
                .expect("actual reader translation")
        })
        .collect::<Vec<_>>();
    lines
        .iter()
        .find_map(|line| {
            let value: Value = serde_json::from_str(line).expect("normalized record");
            (value["method"] == "item/started").then(|| {
                value["params"]["item"]["id"]
                    .as_str()
                    .expect("native item ID")
                    .to_string()
            })
        })
        .expect("started item")
}
