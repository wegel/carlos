//! Drive approval lifetimes through the production event dispatcher with a recording backend.

use std::collections::HashSet;
use std::sync::Mutex;
use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use serde_json::{json, Value};

use super::state::AppState;
use super::view_models::TerminalSize;
use crate::backend::{BackendClient, BackendKind, EventReceiver, RewindForkRequest};
use crate::event::UiEvent;
use crate::session::approval_state::ApprovalQueue;
use crate::session::entry_id::EntryId;

pub(super) struct Client {
    pub(super) kind: BackendKind,
    pub(super) calls: Mutex<Vec<Value>>,
    first_accepts: Mutex<HashSet<EntryId>>,
}

impl Client {
    pub(super) fn new(kind: BackendKind) -> Self {
        Self {
            kind,
            calls: Mutex::new(Vec::new()),
            first_accepts: Mutex::new(HashSet::new()),
        }
    }

    pub(super) fn record(&self, value: Value) {
        self.calls.lock().expect("backend log").push(value);
    }

    pub(super) fn answers(&self) -> Vec<Value> {
        self.calls
            .lock()
            .expect("backend log")
            .iter()
            .filter(|call| call[0] == "answer")
            .cloned()
            .collect()
    }
}

impl BackendClient for Client {
    fn kind(&self) -> BackendKind {
        self.kind
    }
    fn call(&self, method: &str, params: Value, _: Duration) -> anyhow::Result<String> {
        self.record(json!([method, params]));
        Ok(
            json!({"result":{"thread":{"id":"fork","turns":[]},"turn":{"id":"new-turn"}}})
                .to_string(),
        )
    }
    fn respond(&self, id: &Value, answer: Value) -> anyhow::Result<()> {
        self.record(json!(["answer", id, answer]));
        Ok(())
    }
    fn respond_error(&self, id: &Value, code: i64, message: &str) -> anyhow::Result<()> {
        self.record(json!(["error", id, code, message]));
        Ok(())
    }
    fn request_native_control(&self, method: &str, params: Value) -> anyhow::Result<()> {
        self.record(json!([method, params]));
        Ok(())
    }
    fn fork_from_rewind(
        &self,
        thread: &str,
        _: RewindForkRequest,
        _: Duration,
    ) -> anyhow::Result<String> {
        self.record(json!(["fork", thread]));
        Ok(json!({"result":{"thread":{"id":"fork","turns":[]}}}).to_string())
    }
    fn reconnect(&self, thread: &str) -> anyhow::Result<u64> {
        self.record(json!(["reconnect", thread]));
        Ok(2)
    }
    fn take_events_rx(&mut self) -> anyhow::Result<EventReceiver> {
        anyhow::bail!("unused")
    }
    fn stop(&mut self) {}
}

pub(super) fn app(backend: BackendKind) -> AppState {
    let mut app = AppState::new("thread".into());
    app.claude.enabled = backend == BackendKind::Claude;
    app.codex.enabled = backend == BackendKind::Codex;
    app.claude.generation = 1;
    app.claude.phase = super::claude_state::ClaudePhase::Ready;
    app
}

pub(super) fn event(client: &Client, app: &mut AppState, event: UiEvent) {
    super::input::input_dispatch::process_event(
        event,
        client,
        app,
        TerminalSize {
            width: 100,
            height: 30,
        },
        &mut false,
    );
    app.assert_entry_invariants();
}

pub(super) fn key(client: &Client, app: &mut AppState, code: KeyCode, mods: KeyModifiers) {
    event(
        client,
        app,
        UiEvent::Terminal(Event::Key(KeyEvent::new(code, mods))),
    );
}

pub(super) fn server(client: &Client, app: &mut AppState, frame: Value) {
    event(client, app, UiEvent::ServerLine(frame.to_string()));
}

pub(super) fn accept(client: &Client, app: &mut AppState) {
    let first = app.session.approval().pending().is_some_and(|pending| {
        pending
            .facts()
            .requested_permissions
            .as_ref()
            .is_some_and(|p| p["defaultToNo"] == true)
            && client
                .first_accepts
                .lock()
                .expect("consent log")
                .insert(pending.entry_id())
    });
    let before = client.answers();
    let fields = super::native_questions::draft(app).is_some_and(|d| !d.fields.is_empty());
    key(
        client,
        app,
        KeyCode::Char(if fields { 's' } else { 'y' }),
        if fields {
            KeyModifiers::CONTROL
        } else {
            KeyModifiers::NONE
        },
    );
    if first {
        assert_eq!(
            client.answers(),
            before,
            "fresh card must receive its own first accept"
        );
    }
}

pub(super) fn callback(client: &Client, app: &mut AppState, id: &str, kind: &str, payload: Value) {
    server(
        client,
        app,
        json!({"method":"claude/callback","params":{
        "generation":app.claude.generation,"callbackId":id,"kind":kind,"payload":payload}}),
    );
}

pub(super) fn fill(client: &Client, app: &mut AppState) {
    if let Some(draft) = super::native_questions::draft(app) {
        if !draft.fields.is_empty() {
            if draft.fields[0].options.is_empty() {
                event(client, app, UiEvent::Terminal(Event::Paste("2".into())));
            } else {
                key(client, app, KeyCode::Char(' '), KeyModifiers::NONE);
            }
        }
    }
}

pub(super) fn snapshot(client: &Client, app: &AppState) -> Value {
    json!({"calls":*client.calls.lock().expect("backend log"),
        "card":super::view_hint_support::card(app),
        "pending":app.session.approval().pending().map(|p| &p.facts().request_id),
        "details":app.session.approval().pending()
            .map(|p| app.approval_detail_lines(p).collect::<Vec<_>>()),
        "queue":app.session.approval().queued(ApprovalQueue::Claude).iter().chain(app.session.approval().queued(ApprovalQueue::Codex).iter())
            .map(|p| &p.facts().request_id).collect::<Vec<_>>()})
}

pub(super) fn payloads() -> Vec<(&'static str, Value)> {
    vec![
        (
            "permission",
            json!({"toolName":"Bash","input":{"command":"echo old"}}),
        ),
        (
            "permission",
            json!({"toolName":"ExitPlanMode","input":{"plan":"Plan","planFilePath":"plan.txt"}}),
        ),
        (
            "permission",
            json!({"toolName":"AskUserQuestion","input":{"questions":[{
            "question":"Color?","header":"Color","options":[{"label":"Blue"}]}]}}),
        ),
        (
            "elicitation",
            json!({"serverName":"server","requestedSchema":{"type":"object","properties":{}}}),
        ),
        (
            "elicitation",
            json!({"serverName":"server","requestedSchema":{"type":"object",
            "properties":{"count":{"type":"integer"}},"required":["count"]}}),
        ),
        (
            "elicitation",
            json!({"serverName":"server","mode":"url","url":"https://example.com/path","urlHost":"example.com"}),
        ),
        (
            "elicitation",
            json!({"serverName":"server","requestedSchema":{"type":"object",
            "properties":{"nested":{"type":"object"}}}}),
        ),
    ]
}

pub(super) const METHODS: [&str; 7] = [
    "item/commandExecution/requestApproval",
    "item/fileChange/requestApproval",
    "item/permissions/requestApproval",
    "execCommandApproval",
    "applyPatchApproval",
    "item/tool/requestUserInput",
    "mcpServer/elicitation/request",
];

pub(super) fn request(client: &Client, app: &mut AppState, method: &str, id: &str) {
    server(
        client,
        app,
        json!({"method":method,"id":id,"params":{
        "threadId":app.session.thread_id(),"turnId":"turn","itemId":"item",
        "conversationId":app.session.thread_id(),"callId":"item","permissions":{},
        "command":"echo different","reason":"reason","plan":"Plan","serverName":"server",
        "mode":"form","requestedSchema":{"type":"object","properties":{}},
        "questions":[{"id":"q","header":"Header","question":"Color?","options":[{"label":"Blue"}]}]}}),
    );
}
