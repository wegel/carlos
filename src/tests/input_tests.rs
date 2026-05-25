use crossterm::event::KeyEvent;
use serde_json::Value;
use std::sync::mpsc;
#[cfg(feature = "dictation")]
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use super::{
    apply_mobile_mouse_scroll, can_submit_queued_turn, consume_mobile_mouse_char,
    context_label_reserved_cells, context_usage_label, decide_mouse_drag_mode,
    handle_terminal_event, is_key_press_like, is_mobile_mouse_key_candidate, is_newline_enter,
    is_priority_server_line, json, kitt_head_index, normalize_pasted_text,
    parse_mobile_mouse_coords, parse_repeated_plain_mobile_pair, prioritize_events, visual_width,
    AppState, ContextUsage, DictationPhase, DictationProfileState, DurationSamples, Event, KeyCode,
    KeyEventKind, KeyModifiers, MobileMouseConsume, MouseDragMode, PerfMetrics, Role, TerminalSize,
    UiEvent,
};
use crate::backend::{BackendClient, BackendKind, RewindForkRequest};

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

    fn take_events_rx(&mut self) -> anyhow::Result<mpsc::Receiver<String>> {
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

    fn take_events_rx(&mut self) -> anyhow::Result<mpsc::Receiver<String>> {
        anyhow::bail!("unused in test")
    }

    fn stop(&mut self) {}
}

fn usable_dictation_profile() -> DictationProfileState {
    DictationProfileState {
        id: "en".to_string(),
        name: "English".to_string(),
        model_label: Some("/tmp/model.bin".to_string()),
        model_usable: true,
        #[cfg(feature = "dictation")]
        model_path: Some(std::path::PathBuf::from("/tmp/model.bin")),
        #[cfg(feature = "dictation")]
        language: Some("en".to_string()),
        #[cfg(feature = "dictation")]
        vocabulary: None,
    }
}

#[cfg(feature = "dictation")]
fn usable_dictation_profile_with(id: &str, name: &str) -> DictationProfileState {
    DictationProfileState {
        id: id.to_string(),
        name: name.to_string(),
        model_label: Some(format!("/tmp/{id}.bin")),
        model_usable: true,
        model_path: Some(std::path::PathBuf::from(format!("/tmp/{id}.bin"))),
        language: Some(id.to_string()),
        vocabulary: None,
    }
}

include!("input_tests/part_1.rs");

include!("input_tests/part_2.rs");

include!("input_tests/part_3.rs");
