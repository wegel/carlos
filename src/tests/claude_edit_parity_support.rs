//! Keep branch fixtures and backend replies fixed for edit parity checks.

use std::cell::{Cell, RefCell};
use std::time::Duration;

use serde_json::{json, Value};

use super::state::AppState;
use crate::backend::{BackendClient, BackendKind, EventReceiver, PromptOrigin, PromptPriority};

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
    app.claude.enabled = true;
    app
}

pub(super) fn event(app: &mut AppState, method: &str, mut params: Value) {
    params["generation"] = json!(app.claude.generation);
    let _ = super::notifications::handle_server_message_line(
        app,
        &json!({"method":method,"params":params}).to_string(),
    );
}

pub(super) fn advance(app: &mut AppState, stage: usize) {
    super::claude_edit::begin(app, "edited text".into());
    if stage >= 1 {
        super::claude_session_history::switch_session(app, 2, "branch");
    }
    if stage >= 2 {
        super::claude_models::apply_capabilities(app, &json!({"models":[]}));
    }
    if stage >= 3 {
        let input = app
            .dequeue_turn_input(std::time::Instant::now())
            .expect("queued edit");
        super::claude_edit::dispatching(app, &input.text, input.origin);
    }
    if stage >= 4 {
        super::claude_edit::bind_submission(
            app,
            "edited text",
            PromptOrigin::Human,
            &json!({"result":{"turn":{"id":"edit-uuid"}}}),
        );
    }
}

pub(super) fn notices(app: &AppState) -> Vec<&str> {
    app.session
        .messages()
        .iter()
        .map(|m| m.text.as_str())
        .collect()
}

pub(super) fn load(app: &mut AppState) {
    super::claude_session_history::load_unsent(app);
}
