//! Keep optional-id item handling equal to the pre-plan commit, including unkeyed output.

use serde_json::{json, Value};

use crate::app::notifications::handle_server_message_line;
use crate::app::state::AppState;

#[test]
fn idless_items_keep_pre_plan_rows_for_every_backend_and_item_label() {
    let expected: Value =
        serde_json::from_str(include_str!("fixtures/idless_pre_plan.json")).expect("pre-plan rows");
    assert_eq!(all_cases(), expected);
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
    let mut app = AppState::new("thread".into());
    app.codex.enabled = backend == "codex";
    app.claude.enabled = backend == "claude";
    app.summaries.enabled = false;
    let item = item(kind, content, id);
    handle_server_message_line(
        &mut app,
        &json!({"method":method,"params":{"item":item}}).to_string(),
    );
    let messages = &app.messages;
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
