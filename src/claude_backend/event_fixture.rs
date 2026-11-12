//! Fixed native frames and complete normalized record expectations for receiver tests.

use serde_json::{json, Value};

use super::event_test_support::{expected, sdk, stream, SESSION};

pub(super) fn live_frames() -> Vec<Value> {
    vec![
        sdk(
            json!({"type":"system","subtype":"init","session_id":SESSION,"user_message_uuid":"$prompt"}),
        ),
        stream(json!({"type":"message_start","message":{"id":"native-answer"}})),
        stream(
            json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":"why"}}),
        ),
        stream(
            json!({"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":" now"}}),
        ),
        stream(json!({"type":"content_block_stop","index":0})),
        stream(
            json!({"type":"content_block_start","index":1,"content_block":{"type":"text","text":"hello"}}),
        ),
        stream(
            json!({"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":" there"}}),
        ),
        stream(json!({"type":"content_block_stop","index":1})),
        sdk(
            json!({"type":"assistant","uuid":"snapshot","message":{"id":"native-answer","role":"assistant","content":[{"type":"thinking","thinking":"why now"},{"type":"text","text":"hello there"}]}}),
        ),
        sdk(
            json!({"type":"assistant","uuid":"snapshot","message":{"id":"native-answer","role":"assistant","content":[{"type":"text","text":"duplicate"}]}}),
        ),
        sdk(
            json!({"type":"assistant","uuid":"tool-snapshot","message":{"id":"native-tool","role":"assistant","content":[{"type":"tool_use","id":"call","name":"Bash","input":{"command":"pwd"}}]}}),
        ),
        sdk(
            json!({"type":"user","session_id":SESSION,"message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"call","content":"/work"}]}}),
        ),
        json!({"v":1,"type":"callback","callbackId":"permission","kind":"permission","payload":{"toolName":"Bash","input":{"command":"pwd"}}}),
        sdk(
            json!({"type":"result","subtype":"success","is_error":false,"user_message_uuid":"$prompt"}),
        ),
        sdk(json!({"type":"system","subtype":"session_state_changed","state":"idle"})),
    ]
}

pub(super) fn live_records(id: &str) -> Vec<Value> {
    let mut records = vec![
        expected(
            "claude/notice",
            json!({"type":"system","subtype":"init","session_id":SESSION,"user_message_uuid":id}),
        ),
        expected(
            "claude/initialized",
            json!({"session_id":SESSION,"model":null}),
        ),
        expected("claude/promptsConsumed", json!({"ids":[id]})),
        expected("turn/started", json!({"turn":{"id":id}})),
        expected("thread/initialized", json!({"thread":{"id":SESSION}})),
    ];
    records.extend(item_records());
    records.extend(final_records(id));
    records
}

fn item_records() -> Vec<Value> {
    vec![
        item(
            "item/started",
            json!({"item":{"id":"claude-msg-17-1-0","type":"reasoning"}}),
            0,
        ),
        item(
            "item/reasoning/summaryTextDelta",
            json!({"itemId":"claude-msg-17-1-0","delta":"why"}),
            0,
        ),
        item(
            "item/reasoning/summaryTextDelta",
            json!({"itemId":"claude-msg-17-1-0","delta":" now"}),
            0,
        ),
        item(
            "item/completed",
            json!({"item":{"id":"claude-msg-17-1-0","type":"reasoning","summary":[{"text":"why now"}]}}),
            0,
        ),
        expected("claude/thinking", json!({"estimated_tokens":null})),
        item(
            "item/started",
            json!({"item":{"id":"claude-msg-17-1-1","type":"agentMessage"}}),
            1,
        ),
        item(
            "item/agentMessage/delta",
            json!({"itemId":"claude-msg-17-1-1","delta":"hello"}),
            1,
        ),
        item(
            "item/agentMessage/delta",
            json!({"itemId":"claude-msg-17-1-1","delta":" there"}),
            1,
        ),
        item(
            "item/completed",
            json!({"item":{"id":"claude-msg-17-1-1","type":"agentMessage","text":"hello there"}}),
            1,
        ),
    ]
}

fn final_records(id: &str) -> Vec<Value> {
    vec![
        expected(
            "item/completed",
            json!({"item":{"id":"call","type":"toolCall","tool":"Bash","name":"Bash","input":{"command":"pwd"}},"sourceKey":{"sessionId":SESSION,"kind":"tool","id":"call","part":"call"}}),
        ),
        expected(
            "item/completed",
            json!({"item":{"id":"call:result","type":"toolResult","command":"pwd","output":"$ pwd\n/work\n\nexit code: 0","isError":false,"name":"Bash","tool":"Bash","carlosCommandEcho":true},"sourceKey":{"sessionId":SESSION,"kind":"tool","id":"call","part":"result"}}),
        ),
        expected(
            "claude/callback",
            json!({"callbackId":"permission","kind":"permission","payload":{"toolName":"Bash","input":{"command":"pwd"}}}),
        ),
        expected("claude/result", json!({"error":null})),
        expected(
            "claude/activity",
            json!({"type":"system","subtype":"session_state_changed","state":"idle"}),
        ),
        expected(
            "turn/completed",
            json!({"turn":{"id":id,"status":"completed"}}),
        ),
    ]
}

fn item(method: &str, params: Value, block: usize) -> Value {
    let mut record = expected(method, params);
    record["params"]["sourceKey"] =
        json!({"sessionId":SESSION,"kind":"assistant","id":"native-answer","block":block});
    record
}
