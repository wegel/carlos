//! Compare bounded child scalar streaming with the legacy full-string formatter.

use serde_json::{json, Value};

use super::task_activity_text::display_item;

fn text(value: &Value) -> String {
    if value.is_null() {
        "unavailable".into()
    } else {
        value
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| value.to_string())
    }
}

fn reference(item: &Value) -> String {
    match item["type"].as_str() {
        Some("agentMessage" | "plan") => text(&item["text"]),
        Some("commandExecution") => format!(
            "{} [{}]\n{}",
            text(&item["command"]),
            text(&item["status"]),
            text(&item["aggregatedOutput"])
        ),
        _ => serde_json::to_string_pretty(item).expect("native JSON display"),
    }
}

#[test]
fn bounded_scalars_match_old_text_at_every_unicode_boundary() {
    for item in [
        json!({"type":"agentMessage","text":"hello 🌍\n日本語\tend"}),
        json!({"type":"plan","text":null}),
        json!({"type":"commandExecution","command":"printf 🌍","status":"completed","aggregatedOutput":"é\noutput\u{1b}[31m"}),
        json!({"type":"futureTool","nested":{"unicode":"日本語","fields":[1,true,null]}}),
    ] {
        let original = reference(&item);
        for limit in 0..original.len() + 3 {
            let mut end = limit.min(original.len());
            while !original.is_char_boundary(end) {
                end -= 1;
            }
            let display = display_item(&item, limit);
            assert_eq!(display.text, original[..end], "limit {limit}");
            assert_eq!(display.complete, original.len() <= limit);
        }
    }
}
