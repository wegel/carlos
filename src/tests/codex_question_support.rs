//! Test native callbacks with exact IDs while forbidding synthetic user turns.

use std::cell::{Cell, RefCell};
use std::time::Duration;

use anyhow::Result;
use carlos_core::backend::{BackendClient, BackendKind, EventReceiver};
use carlos_core::session::actions::ServerRequestAction;
use carlos_core::session::codex::callbacks::CallbackKey;
use serde_json::{json, Value};

use crate::app::notifications::handle_server_message_line;
use crate::app::state::AppState;

#[derive(Default)]
pub(super) struct Peer {
    pub(super) replies: RefCell<Vec<(Value, Value)>>,
    pub(super) controls: RefCell<Vec<(String, Value)>>,
    pub(super) contexts: RefCell<Vec<Value>>,
    pub(super) fail: Cell<bool>,
    pub(super) allow_turns: Cell<bool>,
    pub(super) calls: RefCell<Vec<(String, Value)>>,
}

impl BackendClient for Peer {
    fn kind(&self) -> BackendKind {
        BackendKind::Codex
    }
    fn call(&self, method: &str, params: Value, _timeout: Duration) -> Result<String> {
        if self.allow_turns.get() && method == "turn/start" {
            self.calls.borrow_mut().push((method.into(), params));
            return Ok(
                json!({"result":{"turn":{"id":"native-turn","status":"inProgress"}}}).to_string(),
            );
        }
        panic!("No synthetic request is allowed: {method}")
    }
    fn respond(&self, id: &Value, result: Value) -> Result<()> {
        anyhow::ensure!(!self.fail.get(), "injected write failure");
        self.replies.borrow_mut().push((id.clone(), result));
        Ok(())
    }
    fn respond_error(&self, _id: &Value, _code: i64, _message: &str) -> Result<()> {
        unimplemented!()
    }
    fn request_native_control(&self, method: &str, params: Value) -> Result<()> {
        self.controls.borrow_mut().push((method.into(), params));
        Ok(())
    }
    fn request_control_with_context(
        &self,
        method: &str,
        params: Value,
        context: Value,
    ) -> Result<()> {
        self.contexts.borrow_mut().push(context);
        self.request_native_control(method, params)
    }
    fn take_events_rx(&mut self) -> Result<EventReceiver> {
        unimplemented!()
    }
    fn stop(&mut self) {}
}

pub(super) fn app() -> AppState {
    let mut app = AppState::new("root".into());
    app.session.set_codex_enabled_for_test(true);
    app.session.enable_native_history();
    app
}

pub(super) fn question(id: Value, thread: &str, blocking: bool) -> Value {
    json!({"id":id,"method":"item/tool/requestUserInput","params":{"threadId":thread,"turnId":"request-turn","itemId":"question-tool","isBlocking":blocking,
        "questions":[{"id":"native-q-7","header":"Native question","question":"Do not use this prose as the answer key","isSecret":false,"options":null}]}})
}

pub(super) fn receive(app: &mut AppState, frame: Value) -> Option<ServerRequestAction> {
    handle_server_message_line(app, &frame.to_string())
}

pub(super) fn field_text(app: &AppState, id: Value) -> String {
    app.view
        .codex_callbacks
        .drafts
        .get(&CallbackKey::new(&id).expect("typed id"))
        .expect("native callback")
        .fields[0]
        .editor
        .lines()
        .join("\n")
}
