//! Keep branch fixtures and backend replies fixed for edit parity checks.

use std::cell::{Cell, RefCell};
use std::time::Duration;

use carlos_core::backend::{
    BackendClient, BackendKind, EventReceiver, PromptOrigin, PromptPriority,
};
use serde_json::{json, Value};

use super::state::AppState;

#[derive(Default)]
pub(super) struct Peer {
    pub(super) calls: RefCell<Vec<(String, PromptOrigin, PromptPriority)>>,
    pub(super) fail: Cell<bool>,
}

impl BackendClient for Peer {
    fn kind(&self) -> BackendKind {
        BackendKind::Claude
    }
    fn call(&self, _: &str, _: Value, _: Duration) -> anyhow::Result<String> {
        panic!("submit_prompt records calls instead")
    }
    fn submit_prompt(
        &self,
        _: &str,
        params: Value,
        origin: PromptOrigin,
        priority: PromptPriority,
        _: Duration,
    ) -> anyhow::Result<String> {
        self.calls.borrow_mut().push((
            params["input"][0]["text"].as_str().expect("text").into(),
            origin,
            priority,
        ));
        anyhow::ensure!(!self.fail.get(), "fixed send failure");
        let id = if params["input"][0]["text"] == "/clear" {
            "clear-uuid"
        } else {
            "edit-uuid"
        };
        Ok(json!({"result":{"turn":{"id":id}}}).to_string())
    }
    fn respond(&self, _: &Value, _: Value) -> anyhow::Result<()> {
        unreachable!()
    }
    fn respond_error(&self, _: &Value, _: i64, _: &str) -> anyhow::Result<()> {
        unreachable!()
    }
    fn take_events_rx(&mut self) -> anyhow::Result<EventReceiver> {
        unreachable!()
    }
    fn stop(&mut self) {}
}

pub(super) fn app() -> AppState {
    let mut app = AppState::new("session".into());
    app.session.set_claude_enabled_for_test(true);
    app
}
