//! Keep native Claude paging, fragments and notices inside Core without an embedded terminal.

mod fragments;
mod notices;
mod pages;

use serde_json::{json, Value};

use super::actions::ServerRequestAction;
use super::backend_test_support::{action, context, normalize};
use super::backend_update::BackendUpdate;
use super::state::SessionState;
use crate::claude_backend::FixtureNormalizer;

fn new_session(root: &str) -> SessionState {
    SessionState::new(root.into(), Default::default(), false)
}

fn feed(core: &mut SessionState, method: &str, params: &Value) -> BackendUpdate {
    core.handle_backend_line(
        &json!({"method":method,"params":params}).to_string(),
        &context(core),
    )
}

fn named(core: &mut SessionState, method: &str, params: &Value) -> Option<ServerRequestAction> {
    action(feed(core, method, params))
}

fn native(core: &mut SessionState, decoder: &mut FixtureNormalizer, request: &str, result: Value) {
    let _ = normalize(
        core,
        decoder,
        &json!({"method":"claude/controller","params":{
            "generation":1,"requestMethod":request,"frame":{"type":"reply","ok":true,"result":result}
        }}),
    );
}

fn record(session: &str, uuid: &str, source: &str, text: &str) -> Value {
    json!({"source":source,"message":{"type":"user","session_id":session,"uuid":uuid,
        "opaque":{"kept":true},"message":{"role":"user","content":text}}})
}

fn control(request: Option<ServerRequestAction>, expected: &str, params: Value) {
    match request.expect("native control") {
        ServerRequestAction::ClaudeControl {
            method,
            params: actual,
        } => {
            assert_eq!(method, expected);
            assert_eq!(actual, params);
        }
        _ => panic!("expected native Claude control"),
    }
}

fn show(core: &mut SessionState, key: String, text: String) {
    core.record_claude_notice(key, &text);
}

fn usage(core: &mut SessionState, facts: &Value) {
    let _ = named(core, "claude/usageReport", facts);
}
