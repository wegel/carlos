//! Keep native SDK completion, callback and metadata facts in Core without a terminal owner.

mod completion;
mod metadata;

use serde_json::{json, Value};

use super::backend_test_support::normalize;
use super::state::SessionState;
use crate::claude_backend::FixtureNormalizer;

fn frame(
    core: &mut SessionState,
    native: &mut FixtureNormalizer,
    frame: Value,
    method: Option<&str>,
) {
    let _ = normalize(
        core,
        native,
        &json!({"method":"claude/controller", "params":{
        "frame":frame,"requestMethod":method}}),
    );
}

fn sdk(core: &mut SessionState, native: &mut FixtureNormalizer, message: Value) {
    frame(
        core,
        native,
        json!({"v":1,"type":"event","event":"sdk","message":message}),
        None,
    );
}

fn submitted(core: &mut SessionState, native: &mut FixtureNormalizer, id: &str) {
    core.record_turn_started(id);
    let _ = normalize(
        core,
        native,
        &json!({"method":"claude/promptSubmitted","params":{"uuid":id}}),
    );
}

fn idle(core: &mut SessionState, native: &mut FixtureNormalizer) {
    sdk(
        core,
        native,
        json!({"type":"system","subtype":"session_state_changed","state":"idle"}),
    );
}

fn result(id: &str) -> Value {
    json!({"type":"result","subtype":"success","is_error":false,"user_message_uuid":id})
}
