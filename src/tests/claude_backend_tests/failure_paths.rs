// Tests stream identity guards.

#[test]
fn session_misroute_event_surfaces_as_error_with_expected_and_actual_ids() {
    let payload = session_misroute_event("expected-id", "actual-id");
    let value: Value = serde_json::from_str(&payload).expect("event json");

    assert_eq!(value["method"].as_str(), Some("error"));
    assert_eq!(
        value["params"]["expectedSessionId"].as_str(),
        Some("expected-id")
    );
    assert_eq!(
        value["params"]["actualSessionId"].as_str(),
        Some("actual-id")
    );
    let message = value["params"]["message"]
        .as_str()
        .expect("error message text");
    assert!(message.contains("expected-id"));
    assert!(message.contains("actual-id"));
}

#[test]
fn live_claude_item_ids_are_unique_across_process_respawns() {
    let lines = &[
        r#"{"type":"stream_event","event":{"type":"message_start","message":{"id":"msg_1","role":"assistant","type":"message","content":[]}}}"#,
        r#"{"type":"stream_event","event":{"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}}"#,
        r#"{"type":"stream_event","event":{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"hello"}}}"#,
        r#"{"type":"stream_event","event":{"type":"content_block_stop","index":1}}"#,
    ];

    let first = collect_live_forwarded_lines_for_test(lines);
    let second = collect_live_forwarded_lines_for_test(lines);

    let first_item_id = first
        .iter()
        .find_map(|line| {
            let value = serde_json::from_str::<Value>(line).ok()?;
            (value.get("method").and_then(Value::as_str) == Some("item/started"))
                .then(|| value["params"]["item"]["id"].as_str().map(str::to_string))
                .flatten()
        })
        .expect("first item id");
    let second_item_id = second
        .iter()
        .find_map(|line| {
            let value = serde_json::from_str::<Value>(line).ok()?;
            (value.get("method").and_then(Value::as_str) == Some("item/started"))
                .then(|| value["params"]["item"]["id"].as_str().map(str::to_string))
                .flatten()
        })
        .expect("second item id");

    assert_ne!(first_item_id, second_item_id);
}
