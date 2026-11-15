//! Drive controller receiver ordering through real Core reducers without the deleted native/UI loop.

mod failures;
mod live;

use std::time::Duration;

use serde_json::{json, Value};

use super::backend_test_support::receive;
use super::state::SessionState;
use crate::backend::{BackendClient, EventReceiver};

fn native_sdk(message: Value) -> Value {
    json!({"v":1,"type":"event","event":"sdk","message":message})
}

fn drain_probe_through_dispatch(
    client: &dyn BackendClient,
    receiver: &EventReceiver,
    state: &mut SessionState,
) {
    client
        .request_native_control("probe", json!({}))
        .expect("queue probe");
    reader_barrier(client);
    dispatch_received(client, state, receiver.try_iter().collect());
}

fn dispatch_received(client: &dyn BackendClient, state: &mut SessionState, lines: Vec<String>) {
    for line in lines {
        receive(state, client, &line);
    }
}

fn dispatch_until_eof(
    client: &dyn BackendClient,
    receiver: &EventReceiver,
    state: &mut SessionState,
) {
    loop {
        let line = receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("EOF fact");
        let value: Value = serde_json::from_str(&line).expect("normalized JSON");
        let done = value["method"] == "claude/error";
        receive(state, client, &line);
        if done {
            break;
        }
    }
}

fn reader_barrier(client: &dyn BackendClient) {
    client
        .call(
            "thread/name/set",
            json!({"threadId":"session","name":"barrier"}),
            Duration::from_secs(5),
        )
        .expect("reader barrier");
}
