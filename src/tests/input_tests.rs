//! Input subsystem test tree: dictation, approvals/rewind, and mobile mouse/perf.

use std::sync::Mutex;
use std::time::Duration;

use carlos_core::backend::{BackendClient, BackendKind, EventReceiver, RewindForkRequest};
use carlos_core::session::context_usage::ContextUsage;
use carlos_core::session::models::Role;
use crossterm::event::KeyEvent;
use serde_json::Value;

use super::{
    apply_mobile_mouse_scroll, can_submit_queued_turn, consume_mobile_mouse_char,
    context_usage_label, decide_mouse_drag_mode, is_key_press_like, is_mobile_mouse_key_candidate,
    is_newline_enter, is_priority_server_line, json, kitt_head_index, normalize_pasted_text,
    parse_mobile_mouse_coords, parse_repeated_plain_mobile_pair, prioritize_events, AppState,
    DurationSamples, Event, KeyCode, KeyEventKind, KeyModifiers, MobileMouseConsume, MouseDragMode,
    PerfMetrics, TerminalSize, UiEvent,
};

struct ApprovalRespondMock {
    responses: std::sync::Mutex<Vec<(Value, Value)>>,
}

impl ApprovalRespondMock {
    fn new() -> Self {
        Self {
            responses: std::sync::Mutex::new(Vec::new()),
        }
    }
}

impl BackendClient for ApprovalRespondMock {
    fn kind(&self) -> BackendKind {
        BackendKind::Claude
    }

    fn call(&self, _method: &str, _params: Value, _timeout: Duration) -> anyhow::Result<String> {
        anyhow::bail!("unused in test")
    }

    fn respond(&self, request_id: &Value, result: Value) -> anyhow::Result<()> {
        self.responses
            .lock()
            .expect("responses lock")
            .push((request_id.clone(), result));
        Ok(())
    }

    fn respond_error(&self, _request_id: &Value, _code: i64, _message: &str) -> anyhow::Result<()> {
        anyhow::bail!("unused in test")
    }

    fn take_events_rx(&mut self) -> anyhow::Result<EventReceiver> {
        anyhow::bail!("unused in test")
    }

    fn stop(&mut self) {}
}

struct CodexRewindMock {
    fork_calls: Mutex<Vec<(String, RewindForkRequest)>>,
    turn_start_calls: Mutex<Vec<Value>>,
    fork_response: String,
}

impl CodexRewindMock {
    fn new(fork_response: String) -> Self {
        Self {
            fork_calls: Mutex::new(Vec::new()),
            turn_start_calls: Mutex::new(Vec::new()),
            fork_response,
        }
    }
}

impl BackendClient for CodexRewindMock {
    fn kind(&self) -> BackendKind {
        BackendKind::Codex
    }

    fn call(&self, method: &str, params: Value, _timeout: Duration) -> anyhow::Result<String> {
        assert_eq!(method, "turn/start");
        self.turn_start_calls
            .lock()
            .expect("turn start calls lock")
            .push(params);
        Ok("{\"jsonrpc\":\"2.0\",\"result\":{}}".to_string())
    }

    fn fork_from_rewind(
        &self,
        thread_id: &str,
        request: RewindForkRequest,
        _timeout: Duration,
    ) -> anyhow::Result<String> {
        self.fork_calls
            .lock()
            .expect("fork calls lock")
            .push((thread_id.to_string(), request));
        Ok(self.fork_response.clone())
    }

    fn respond(&self, _request_id: &Value, _result: Value) -> anyhow::Result<()> {
        anyhow::bail!("unused in test")
    }

    fn respond_error(&self, _request_id: &Value, _code: i64, _message: &str) -> anyhow::Result<()> {
        anyhow::bail!("unused in test")
    }

    fn take_events_rx(&mut self) -> anyhow::Result<EventReceiver> {
        anyhow::bail!("unused in test")
    }

    fn stop(&mut self) {}
}

include!("input_tests/dictation.rs");

include!("input_tests/approvals_and_rewind.rs");

include!("input_tests/mobile_mouse_and_perf.rs");
