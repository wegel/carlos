//! Preserve optional quota read identity across the real controller receiver.

use std::time::Duration;

use serde_json::{json, Value};

use super::event_test_support::client;
use crate::backend::BackendClient;

#[test]
fn an_optional_quota_reply_retains_its_original_read_scope_in_the_normalized_reply() {
    let (client, receiver) = client(Vec::new(), json!({"windows":[]}), 1024);
    let scope = "d".repeat(64);
    client
        .request_native_control(
            "usage.optional",
            json!({"revision":7,"quotaRead":12,"quotaScope":scope}),
        )
        .expect("asynchronous optional quota request");
    let line = receiver
        .recv_timeout(Duration::from_secs(3))
        .expect("quota reply");
    client.acknowledge_event(line.len());
    let reply: Value = serde_json::from_str(&line).expect("normalized reply");
    assert_eq!(reply["method"], "claude/controlReply");
    assert_eq!(reply["params"]["requestMethod"], "usage.optional");
    assert_eq!(reply["params"]["requestContext"]["revision"], 7);
    assert_eq!(reply["params"]["requestContext"]["quotaRead"], 12);
    assert_eq!(reply["params"]["requestContext"]["quotaScope"], scope);
}
