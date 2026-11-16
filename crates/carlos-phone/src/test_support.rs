//! Drive the exported phone API with fixed, protocol-valid daemon replies.

use std::sync::Arc;

use serde_json::{json, Value};

use crate::{PhoneClient, PhoneFrame};

pub(crate) const SESSION: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

pub(crate) fn token() -> String {
    format!("{}.{}", "1".repeat(32), "2".repeat(64))
}

pub(crate) fn hello(id: i64, resumed: bool) -> Vec<u8> {
    serde_json::to_vec(&json!({"jsonrpc":"2.0","id":id,"result":{
        "protocol":1,"client":"3".repeat(32),"instance":"4".repeat(32),
        "resume_token":"5".repeat(64),"next_operation":"1","resumed":resumed,
        "host":"fixture","daemon_version":"0.1.0"
    }}))
    .expect("greeting reply")
}

pub(crate) fn request(frame: &PhoneFrame) -> Value {
    serde_json::from_str(&frame.payload).expect("request")
}

pub(crate) fn connected() -> Arc<PhoneClient> {
    let client = PhoneClient::new();
    let frame = client.begin("phone".into(), token()).expect("begin");
    assert!(
        client
            .receive(frame.generation, hello(frame.request_id, false))
            .expect("hello")
            .ready
    );
    client
}

pub(crate) fn reply(client: &PhoneClient, frame: &PhoneFrame, result: Value) -> crate::PhoneBatch {
    client
        .receive(
            frame.generation,
            serde_json::to_vec(&json!({
                "jsonrpc":"2.0", "id":frame.request_id, "result":result
            }))
            .expect("reply bytes"),
        )
        .expect("accepted reply")
}

pub(crate) fn info(availability: &str) -> Value {
    json!({"id":SESSION,"name":"Shared session","backend":"claude","model":null,
        "host":"fixture","cwd":"/project","phase":"idle","availability":availability,
        "current_turn":null,"ralph":false,"turn_started_at":null,"context":null,"quota":null})
}

pub(crate) fn subscribed(availability: &str) -> Arc<PhoneClient> {
    let client = connected();
    let watch = client.watch_sessions().expect("watch");
    let changed = reply(&client, &watch, json!({"sessions":[info(availability)]}));
    assert!(changed.catalog_changed);
    let frame = client.subscribe(SESSION.into()).expect("subscribe");
    reply(
        &client,
        &frame,
        json!({"kind":"snapshot","snapshot":{
            "info":info(availability),"entries":[],"approvals":[],"seq":"1","has_older":false
        }}),
    );
    client
}
