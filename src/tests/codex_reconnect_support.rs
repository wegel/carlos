//! Build reconnect snapshots and drive the stable event boundary without a model.

use std::cell::RefCell;
use std::time::Duration;

use anyhow::{bail, Result};
use serde_json::{json, Value};

use super::codex_question_support::Peer as QuestionPeer;
use crate::app::state::AppState;
use crate::backend::{BackendClient, BackendKind};
use crate::codex_snapshot::CodexSnapshot;

#[derive(Default)]
pub(super) struct Peer {
    pub(super) native: QuestionPeer,
    pub(super) snapshot: RefCell<Option<CodexSnapshot>>,
    pub(super) acknowledged: RefCell<Vec<usize>>,
    pub(super) aborted: RefCell<usize>,
}

impl BackendClient for Peer {
    fn kind(&self) -> BackendKind {
        BackendKind::Codex
    }
    fn call(&self, _: &str, _: Value, _: Duration) -> Result<String> {
        bail!("not a blocking test")
    }
    fn respond(&self, id: &Value, value: Value) -> Result<()> {
        self.native.respond(id, value)
    }
    fn respond_error(&self, id: &Value, code: i64, message: &str) -> Result<()> {
        self.native.respond_error(id, code, message)
    }
    fn request_rate_limits(&self) -> Result<Option<String>> {
        self.native
            .controls
            .borrow_mut()
            .push(("account/rateLimits/read".into(), json!({})));
        Ok(Some("quota".into()))
    }
    fn request_control_with_context(
        &self,
        method: &str,
        params: Value,
        context: Value,
    ) -> Result<()> {
        self.native
            .request_control_with_context(method, params, context)
    }
    fn take_reconnect_snapshot(&self, _: u64) -> Option<CodexSnapshot> {
        self.snapshot.borrow_mut().take()
    }
    fn acknowledge_event(&self, length: usize) {
        self.acknowledged.borrow_mut().push(length);
    }
    fn abort_connection(&self) {
        *self.aborted.borrow_mut() += 1;
    }
    fn take_events_rx(&mut self) -> Result<std::sync::mpsc::Receiver<String>> {
        bail!("test has no pipe")
    }
    fn stop(&mut self) {}
}

pub(super) fn snapshot(text: &str, status: &str) -> CodexSnapshot {
    CodexSnapshot {
        thread: json!({"id":"root","status":{"type":status},"turns":[{
            "id":"turn","status":"inProgress","items":[{"id":"answer","type":"agentMessage","text":text}]}]}),
        runtime: json!({"result":{"thread":{"id":"root"},"model":"a","reasoningEffort":"low"}})
            .to_string(),
        catalog: json!({"data":[{"id":"a","model":"a","displayName":"a","isDefault":true}]}),
        queue: Some(json!([])),
        goal: Some(json!({"goal":null})),
        panel: json!({"agents":[],"processes":[],"older":null}),
    }
}

pub(super) fn app() -> AppState {
    let mut app = super::codex_question_support::app();
    app.codex.closed = true;
    app.codex.connection.generation = 2;
    app.codex.connection.rebuilding = true;
    app.set_input_text("preserve main 日本語");
    app
}

pub(super) fn receive(peer: &Peer, app: &mut AppState, generation: u64, event: Value) -> usize {
    let line =
        json!({"method":"carlos/codex/frame","params":{"generation":generation,"event":event}})
            .to_string();
    assert!(crate::app::codex_reconnect::receive(peer, app, &line));
    line.len()
}

pub(super) fn ready(peer: &Peer, app: &mut AppState, snapshot: CodexSnapshot) {
    *peer.snapshot.borrow_mut() = Some(snapshot);
    receive(
        peer,
        app,
        2,
        json!({"method":"carlos/codex/ready","params":{"generation":2,"threadId":"root"}}),
    );
}

pub(super) fn delta(text: &str) -> Value {
    json!({"method":"item/agentMessage/delta","params":{"threadId":"root","turnId":"turn","itemId":"answer","delta":text}})
}

pub(super) fn complete(text: &str) -> Value {
    json!({"method":"item/completed","params":{"threadId":"root","turnId":"turn","item":{"id":"answer","type":"agentMessage","text":text}}})
}
