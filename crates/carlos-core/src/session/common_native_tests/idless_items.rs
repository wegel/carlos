//! Keep legacy optional-id rows equal and preserve newly supported native tool sources.

use serde_json::{json, Value};

use super::{feed as handle_server_message_line, session};

#[test]
fn idless_items_keep_pre_plan_rows_for_every_backend_and_item_label() {
    let expected: Value =
        serde_json::from_str(include_str!("fixtures/idless_pre_plan.json")).expect("pre-plan rows");
    assert_eq!(legacy_cases(all_cases()), legacy_cases(expected));
}

fn legacy_cases(cases: Value) -> Vec<Value> {
    cases
        .as_array()
        .expect("native cases")
        .iter()
        .filter(|case| {
            !matches!(
                case["item"]["type"].as_str(),
                Some("fileChange" | "mcpToolCall")
            )
        })
        .cloned()
        .collect()
}

#[test]
fn idless_native_file_changes_and_mcp_keep_complete_unkeyed_source() {
    let patch = json!({"type":"fileChange", "status":"completed", "changes":[
        {"path":"one.rs", "diff":"@@ -1 +1 @@\n-old\n+new 🌍\n"},
        {"path":"two.rs", "diff":"@@ -0,0 +1 @@\n+second 🌍\n"}
    ]});
    let mcp = json!({"type":"mcpToolCall", "status":"completed", "tool":"lookup",
        "arguments":{"query":"original 🌍"}, "result":{"structuredContent":{"answer":42}}});
    let mut app = session("thread");
    for item in [&patch, &mcp] {
        handle_server_message_line(
            &mut app,
            &json!({"method":"item/completed",
            "params":{"item":item}})
            .to_string(),
        );
    }
    assert!(
        app.messages().items().is_empty(),
        "id-less rows stay unkeyed"
    );
    assert_eq!(app.messages().len(), 3);
    for (row, change) in app
        .messages()
        .iter()
        .take(2)
        .zip(patch["changes"].as_array().expect("changes"))
    {
        assert_eq!(row.text, change["diff"].as_str().expect("original diff"));
        assert_eq!(row.file_path.as_deref(), change["path"].as_str());
    }
    assert_eq!(
        serde_json::from_str::<Value>(&app.messages()[2].text).expect("complete MCP JSON"),
        mcp
    );
}

pub(super) fn all_cases() -> Value {
    let mut cases = Vec::new();
    for backend in ["codex", "claude", "grok"] {
        for kind in item_kinds() {
            for method in ["item/started", "item/completed"] {
                for content in 0..8 {
                    for id in [None, Some(Value::Null), Some(json!(42))] {
                        cases.push(run_case(backend, kind, method, content, id));
                    }
                }
            }
        }
    }
    json!(cases)
}

fn item_kinds() -> [&'static str; 20] {
    [
        "commandExecution",
        "toolCall",
        "tool_call",
        "toolInvocation",
        "functionCall",
        "mcpToolCall",
        "toolResult",
        "toolOutput",
        "tool_result",
        "functionCallOutput",
        "mcpToolResult",
        "fileChange",
        "reasoning",
        "agentMessage",
        "userMessage",
        "contextCompaction",
        "message",
        "sleep",
        "subAgentActivity",
        "collabAgentToolCall",
    ]
}

fn item(kind: &str, content: usize, id: Option<Value>) -> Value {
    let mut value = json!({"type":kind});
    if let Some(id) = id {
        value["id"] = id;
    }
    match content {
        0 => {}
        1 => value["output"] = json!("hello"),
        2 => {
            value["name"] = json!("Read");
            value["input"] = json!({"file_path":"same.rs"});
        }
        3 => {
            value["command"] = json!("pwd");
            value["aggregatedOutput"] = json!("/tmp");
        }
        4 => value["diff"] = json!("@@ -1 +1 @@\n-a\n+b\n"),
        5 => {
            value["changes"] = json!([
            {"path":"a.rs","diff":"@@ -1 +1 @@\n-a\n+b\n"},
            {"path":"b.rs","diff":"@@ -1 +1 @@\n-c\n+d\n"}])
        }
        6 => {
            value["text"] = json!("answer");
            value["summary"] = json!([{"text":"think"}]);
        }
        7 => value["content"] = json!([{"type":"text","text":"content"}]),
        _ => unreachable!(),
    }
    value
}

fn run_case(backend: &str, kind: &str, method: &str, content: usize, id: Option<Value>) -> Value {
    let mut app = session("thread");
    app.set_codex_enabled_for_test(backend == "codex");
    app.set_claude_enabled_for_test(backend == "claude");
    app.set_summary_enabled_for_test(false);
    let item = item(kind, content, id);
    handle_server_message_line(
        &mut app,
        &json!({"method":method,"params":{"item":item}}).to_string(),
    );
    let messages = app.messages();
    assert!(
        messages.items().is_empty(),
        "id-less rows must stay unkeyed"
    );
    let rows: Vec<_> = messages
        .iter()
        .map(|row| {
            json!({
        "role":format!("{:?}",row.role),"kind":format!("{:?}",row.kind),"text":row.text,
        "path":row.file_path,"tool":row.tool_output.as_ref().map(|t|
            json!({"summary":t.summary,"generated":t.generated,"failed":t.failed}))})
        })
        .collect();
    json!({"backend":backend,"method":method,"item":item,"rows":rows})
}
