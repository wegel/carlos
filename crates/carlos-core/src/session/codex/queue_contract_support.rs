//! Build fixed native queue values while the real queue controller owns every transition.

use serde_json::{json, Value};

use super::queue::{mutate, QueueUpdate};
use crate::session::state::SessionState;
use crate::session::test_backend::TestBackend;

pub(super) fn session() -> SessionState {
    let mut state = SessionState::new("root".into(), Default::default(), false);
    state.enable_codex();
    state.enable_native_history();
    state.configure_codex_connection(2, None);
    state
}

pub(super) fn entry(id: &str, text: &str) -> Value {
    json!({"id":id,"clientUserMessageId":format!("client-{id}"),
        "input":[{"type":"text","text":text,"text_elements":[]}]})
}

pub(super) fn call(peer: &TestBackend, index: usize) -> (String, Value, Value) {
    peer.calls.lock().expect("recorded native calls")[index].clone()
}

pub(super) fn reply(peer: &TestBackend, index: usize, response: Value) -> Value {
    let (method, mut params, context) = call(peer, index);
    if matches!(method.as_str(), "thread/queue/add" | "thread/queue/update") {
        params
            .as_object_mut()
            .expect("native params")
            .remove("input");
    }
    json!({"requestMethod":method,"requestParams":params,
        "requestContext":context,"response":response})
}

pub(super) fn add(text: &str) -> Value {
    json!({"threadId":"root","clientUserMessageId":"client-queued-human",
        "input":[{"type":"text","text":text,"text_elements":[]}]})
}

pub(super) fn submit(
    peer: &TestBackend,
    state: &mut SessionState,
    update: &mut QueueUpdate,
    text: &str,
) -> bool {
    let scope = state.codex_scope();
    mutate(
        peer,
        state,
        scope,
        update,
        "thread/queue/add",
        add(text),
        Some(text.into()),
    )
}
