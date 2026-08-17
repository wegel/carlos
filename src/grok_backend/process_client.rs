//! Replies to agent-originated ACP client requests.

use std::io::Write;
use std::process::ChildStdin;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

pub(super) fn respond_to_agent_request(parsed: &Value, stdin: &Arc<Mutex<ChildStdin>>) {
    let Some(id) = parsed.get("id").cloned() else {
        return;
    };
    let method = parsed.get("method").and_then(Value::as_str).unwrap_or("");
    let reply = match method {
        "session/request_permission" => permission_allow_reply(id, parsed.get("params")),
        _ => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32601, "message": format!("unsupported client method: {method}") },
        }),
    };
    if let Ok(mut stdin) = stdin.lock() {
        let _ = writeln!(stdin, "{reply}");
        let _ = stdin.flush();
    }
}

fn permission_allow_reply(id: Value, params: Option<&Value>) -> Value {
    let option_id = params
        .and_then(|params| params.get("options"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find_map(|option| {
            let kind = option.get("kind").and_then(Value::as_str)?;
            matches!(kind, "allow_once" | "allow_always")
                .then(|| option.get("optionId").and_then(Value::as_str))
                .flatten()
        })
        .unwrap_or("allow-once");
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "outcome": {
                "outcome": "selected",
                "optionId": option_id,
            }
        }
    })
}
