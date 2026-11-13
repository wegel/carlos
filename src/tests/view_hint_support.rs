//! Shared deterministic terminal and backend fixtures for view-owned hints.

use std::sync::Mutex;
use std::time::Duration;

use carlos_core::backend::{BackendClient, BackendKind, EventReceiver};
use carlos_core::session::approval_state::ApprovalRequestDraft;
use carlos_core::session::approval_state::ApprovalRequestKind;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use serde_json::{json, Value};

use super::input_events::handle_terminal_event;
use super::state::AppState;
use super::view_models::TerminalSize;

pub(super) struct Client {
    pub(super) kind: BackendKind,
    pub(super) calls: Mutex<Vec<Value>>,
}

impl Client {
    pub(super) fn new(kind: BackendKind) -> Self {
        Self {
            kind,
            calls: Mutex::new(Vec::new()),
        }
    }
}

impl BackendClient for Client {
    fn kind(&self) -> BackendKind {
        self.kind
    }
    fn call(&self, method: &str, params: Value, _: Duration) -> anyhow::Result<String> {
        self.calls
            .lock()
            .expect("calls")
            .push(json!([method, params]));
        Ok("{\"result\":{}}".into())
    }
    fn respond(&self, id: &Value, answer: Value) -> anyhow::Result<()> {
        self.calls.lock().expect("calls").push(json!([id, answer]));
        Ok(())
    }
    fn respond_error(&self, _: &Value, _: i64, _: &str) -> anyhow::Result<()> {
        panic!("unexpected reply error")
    }
    fn take_events_rx(&mut self) -> anyhow::Result<EventReceiver> {
        anyhow::bail!("unused")
    }
    fn stop(&mut self) {}
}

pub(super) fn key(client: &Client, app: &mut AppState, code: KeyCode, modifiers: KeyModifiers) {
    handle_terminal_event(
        client,
        app,
        Event::Key(KeyEvent::new(code, modifiers)),
        TerminalSize {
            width: 100,
            height: 30,
        },
    );
}

pub(super) fn request(kind: ApprovalRequestKind, id: &str) -> ApprovalRequestDraft {
    carlos_core::session::approval_state::ApprovalRequestDraft {
        request_id: json!(id),
        method: "claude/native".into(),
        kind,
        title: format!("Approve {kind:?}"),
        detail_lines: vec!["native request details".into()],
        requested_permissions: Some(json!({"defaultToNo":true,"toolName":"Bash",
            "requestedSchema":{"type":"object","properties":{}}})),
        can_accept_for_session: true,
        can_decline: true,
        can_cancel: true,
    }
}

pub(super) fn card(app: &AppState) -> String {
    let mut buf = Buffer::empty(Rect::new(0, 0, 100, 30));
    super::overlay_render::draw_approval_overlay(
        &mut buf,
        TerminalSize {
            width: 100,
            height: 30,
        },
        app,
    );
    let cells: Vec<_> = buf
        .content
        .iter()
        .map(|c| format!("{}:{:?}", c.symbol(), c.style()))
        .collect();
    blake3::hash(cells.join("\n").as_bytes())
        .to_hex()
        .to_string()
}

pub(super) fn snapshot(client: &Client, app: &AppState) -> Value {
    json!({"card":card(app),"calls":*client.calls.lock().expect("calls"),
        "confirm":app.view.confirm_allow.is_some(),"pending":app.session.approval().pending().map(|p| &p.facts().request_id)})
}
