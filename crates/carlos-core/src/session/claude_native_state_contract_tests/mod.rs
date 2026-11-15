//! Reduce native reset, process and hidden-child streams independently of private terminal state.

mod process;
mod reset;

use serde_json::{json, Value};

use super::backend_test_support::normalize;
use super::models::Role;
use super::state::SessionState;
use crate::claude_backend::FixtureNormalizer;

fn frame(
    core: &mut SessionState,
    native: &mut FixtureNormalizer,
    frame: Value,
    epoch: u64,
    method: Option<&str>,
    context: Value,
) {
    let _ = normalize(
        core,
        native,
        &json!({"method":"claude/controller","params":{
        "frame":frame,"epoch":epoch,"requestMethod":method,"requestContext":context}}),
    );
}

fn sdk(core: &mut SessionState, native: &mut FixtureNormalizer, message: Value, epoch: u64) {
    frame(
        core,
        native,
        json!({"v":1,"type":"event","event":"sdk","message":message}),
        epoch,
        None,
        Value::Null,
    );
}

fn submitted(core: &mut SessionState, native: &mut FixtureNormalizer, id: &str, text: &str) {
    let index = core.append_message(Role::User, text);
    core.record_prompt_history(text, Some(core.messages()[index].id));
    let key = core.backend_entry_key(&format!("claude-prompt:{id}"));
    core.attach_claude_prompt_echo(id, &key, None);
    let _ = normalize(
        core,
        native,
        &json!({"method":"claude/promptSubmitted","params":{
        "uuid":id,"text":text,"origin":"human","priority":"later"}}),
    );
}

fn init(
    core: &mut SessionState,
    native: &mut FixtureNormalizer,
    id: &str,
    prompts: Value,
    epoch: u64,
) {
    sdk(
        core,
        native,
        json!({"type":"system","subtype":"init","session_id":id,
        "user_message_uuids":prompts}),
        epoch,
    );
}

fn state(core: &mut SessionState, native: &mut FixtureNormalizer, name: &str, epoch: u64) {
    sdk(
        core,
        native,
        json!({"type":"system","subtype":"session_state_changed","state":name}),
        epoch,
    );
}

fn result(core: &mut SessionState, native: &mut FixtureNormalizer, id: &str, epoch: u64) {
    sdk(
        core,
        native,
        json!({"type":"result","subtype":"success","is_error":false,
        "user_message_uuid":id}),
        epoch,
    );
}
