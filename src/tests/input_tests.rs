use crossterm::event::KeyEvent;
use serde_json::Value;
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::Duration;

use super::*;
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

#[test]
fn prioritize_events_handles_terminal_first_and_budgets_server_lines() {
    let mut deferred = std::collections::VecDeque::new();
    let incoming = vec![
        UiEvent::ServerLine("s1".to_string()),
        UiEvent::Terminal(Event::Resize(80, 24)),
        UiEvent::ServerLine("s2".to_string()),
    ];

    let prioritized = prioritize_events(incoming, &mut deferred, 1);
    assert_eq!(prioritized.len(), 2);
    assert!(matches!(
        prioritized[0],
        UiEvent::Terminal(Event::Resize(80, 24))
    ));
    assert!(matches!(prioritized[1], UiEvent::ServerLine(ref s) if s == "s1"));
    assert_eq!(deferred.len(), 1);
    assert_eq!(deferred.front().map(String::as_str), Some("s2"));
}

#[test]
fn ctrl_d_toggles_dictation_recording_to_transcribing() {
    let client = ApprovalRespondMock::new();
    let mut app = AppState::new("thread-1".to_string());
    app.configure_dictation(usable_dictation_profile());
    let size = TerminalSize {
        width: 100,
        height: 30,
    };

    handle_terminal_event(
        &client,
        &mut app,
        Event::Key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL)),
        size,
    );
    assert!(matches!(app.dictation_phase(), DictationPhase::Recording));

    handle_terminal_event(
        &client,
        &mut app,
        Event::Key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL)),
        size,
    );
    assert!(matches!(
        app.dictation_phase(),
        DictationPhase::Transcribing { .. }
    ));
}

#[test]
fn ctrl_d_restarts_dictation_while_transcribing() {
    let client = ApprovalRespondMock::new();
    let mut app = AppState::new("thread-1".to_string());
    app.configure_dictation(usable_dictation_profile());
    let size = TerminalSize {
        width: 100,
        height: 30,
    };

    app.start_dictation_recording();
    app.stop_dictation_recording();
    handle_terminal_event(
        &client,
        &mut app,
        Event::Key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL)),
        size,
    );

    assert!(matches!(app.dictation_phase(), DictationPhase::Recording));
}

#[test]
fn escape_cancels_recording_and_transcribing() {
    let client = ApprovalRespondMock::new();
    let mut app = AppState::new("thread-1".to_string());
    app.configure_dictation(usable_dictation_profile());
    let size = TerminalSize {
        width: 100,
        height: 30,
    };

    app.start_dictation_recording();
    handle_terminal_event(
        &client,
        &mut app,
        Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty())),
        size,
    );
    assert!(matches!(app.dictation_phase(), DictationPhase::Idle));

    app.start_dictation_recording();
    app.stop_dictation_recording();
    handle_terminal_event(
        &client,
        &mut app,
        Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty())),
        size,
    );
    assert!(matches!(app.dictation_phase(), DictationPhase::Idle));
}

#[test]
fn dictation_does_not_start_during_active_turn() {
    let mut app = AppState::new("thread-1".to_string());
    app.configure_dictation(usable_dictation_profile());
    app.active_turn_id = Some("turn-1".to_string());

    app.start_dictation_recording();

    assert!(matches!(app.dictation_phase(), DictationPhase::Idle));
    assert_eq!(app.status, "dictation unavailable while turn is active");
}

#[test]
fn missing_dictation_model_reports_error_without_recording() {
    let mut app = AppState::new("thread-1".to_string());
    app.configure_dictation(DictationProfileState {
        id: "en".to_string(),
        name: "English".to_string(),
        model_label: Some("/tmp/missing-model.bin".to_string()),
        model_usable: false,
        #[cfg(feature = "dictation")]
        model_path: Some(std::path::PathBuf::from("/tmp/missing-model.bin")),
        #[cfg(feature = "dictation")]
        language: Some("en".to_string()),
        #[cfg(feature = "dictation")]
        vocabulary: None,
    });

    app.start_dictation_recording();

    assert!(matches!(app.dictation_phase(), DictationPhase::Idle));
    assert_eq!(
        app.status,
        "dictation model unavailable: /tmp/missing-model.bin"
    );
}

#[test]
fn dictation_final_text_commits_and_returns_to_idle() {
    let mut app = AppState::new("thread-1".to_string());
    app.configure_dictation(usable_dictation_profile());

    app.start_dictation_recording();
    app.stop_dictation_recording();
    app.apply_dictation_partial("draft text");
    app.commit_dictation_final("final text");

    assert!(matches!(app.dictation_phase(), DictationPhase::Idle));
    assert_eq!(app.input_text(), "final text");
    assert_eq!(app.status, "dictation inserted");
}

#[cfg(feature = "dictation")]
#[test]
fn dictation_auto_stop_event_moves_recording_to_transcribing() {
    let mut app = AppState::new("thread-1".to_string());
    app.configure_dictation(usable_dictation_profile());

    app.start_dictation_recording();
    app.handle_dictation_event(
        crate::dictation::events::DictationEvent::CaptureAutoStopped(vec![0.1, 0.2, 0.3]),
    );

    assert!(matches!(
        app.dictation_phase(),
        DictationPhase::Transcribing { .. }
    ));
    assert_eq!(app.last_dictation_audio_len(), Some(3));
    assert_eq!(app.status, "dictation transcribing (3 samples)");
}

#[cfg(feature = "dictation")]
#[test]
fn dictation_capture_error_returns_to_idle() {
    let mut app = AppState::new("thread-1".to_string());
    app.configure_dictation(usable_dictation_profile());

    app.start_dictation_recording();
    app.handle_dictation_event(crate::dictation::events::DictationEvent::CaptureError(
        "device disconnected".to_string(),
    ));

    assert!(matches!(app.dictation_phase(), DictationPhase::Idle));
    assert_eq!(app.status, "dictation capture failed: device disconnected");
}

#[cfg(feature = "dictation")]
#[test]
fn dictation_worker_final_commits_matching_request() {
    let mut app = AppState::new("thread-1".to_string());
    app.configure_dictation(usable_dictation_profile());

    app.start_dictation_recording();
    app.stop_dictation_recording();
    app.dictation_request_id = Some(7);
    app.handle_dictation_event(
        crate::dictation::events::DictationEvent::TranscriptionFinal {
            request_id: 7,
            text: "final dictation".to_string(),
        },
    );

    assert!(matches!(app.dictation_phase(), DictationPhase::Idle));
    assert_eq!(app.input_text(), "final dictation");
    assert_eq!(app.status, "dictation inserted");
}

#[cfg(feature = "dictation")]
#[test]
fn dictation_worker_late_final_is_ignored_after_cancel() {
    let mut app = AppState::new("thread-1".to_string());
    app.configure_dictation(usable_dictation_profile());

    app.start_dictation_recording();
    app.stop_dictation_recording();
    app.dictation_request_id = Some(7);
    app.cancel_dictation();
    app.handle_dictation_event(
        crate::dictation::events::DictationEvent::TranscriptionFinal {
            request_id: 7,
            text: "late text".to_string(),
        },
    );

    assert!(matches!(app.dictation_phase(), DictationPhase::Idle));
    assert_eq!(app.input_text(), "");
}

#[test]
fn dictation_profile_picker_state_is_tracked() {
    let mut app = AppState::new("thread-1".to_string());

    app.open_dictation_profile_picker();
    assert!(app.dictation_profile_picker_open());
    app.close_dictation_profile_picker();
    assert!(!app.dictation_profile_picker_open());
}

#[cfg(feature = "dictation")]
#[test]
fn f7_cycles_dictation_profiles() {
    let client = ApprovalRespondMock::new();
    let mut app = AppState::new("thread-1".to_string());
    app.configure_dictation_profiles(
        vec![
            usable_dictation_profile_with("en", "English"),
            usable_dictation_profile_with("fr", "French"),
        ],
        "en",
    );
    let size = TerminalSize {
        width: 100,
        height: 30,
    };

    handle_terminal_event(
        &client,
        &mut app,
        Event::Key(KeyEvent::new(KeyCode::F(7), KeyModifiers::empty())),
        size,
    );

    app.start_dictation_recording();
    assert_eq!(
        app.dictation_status_label(),
        Some("DICTATING [French]".to_string())
    );
    assert_eq!(app.status, "dictation recording");
}

#[test]
fn prioritize_events_drains_deferred_server_lines_without_new_input() {
    let mut deferred = std::collections::VecDeque::new();
    deferred.push_back("a".to_string());
    deferred.push_back("b".to_string());
    deferred.push_back("c".to_string());

    let prioritized = prioritize_events(Vec::new(), &mut deferred, 2);
    assert_eq!(prioritized.len(), 2);
    assert!(matches!(prioritized[0], UiEvent::ServerLine(ref s) if s == "a"));
    assert!(matches!(prioritized[1], UiEvent::ServerLine(ref s) if s == "b"));
    assert_eq!(deferred.len(), 1);
    assert_eq!(deferred.front().map(String::as_str), Some("c"));
}

#[test]
fn prioritize_events_promotes_turn_completed_even_when_budget_is_zero() {
    let mut deferred = std::collections::VecDeque::new();
    deferred.push_back("low-1".to_string());
    deferred.push_back("{\"method\":\"turn/completed\",\"params\":{}}".to_string());
    deferred.push_back("low-2".to_string());

    let prioritized = prioritize_events(Vec::new(), &mut deferred, 0);
    assert_eq!(prioritized.len(), 1);
    assert!(matches!(
        prioritized[0],
        UiEvent::ServerLine(ref s) if s.contains("\"method\":\"turn/completed\"")
    ));
    assert_eq!(deferred.len(), 2);
    assert_eq!(deferred.front().map(String::as_str), Some("low-1"));
}

#[test]
fn is_priority_server_line_identifies_control_notifications() {
    assert!(is_priority_server_line(
        "{\"method\":\"turn/completed\",\"params\":{}}"
    ));
    assert!(is_priority_server_line(
        "{\"method\":\"turn/started\",\"params\":{}}"
    ));
    assert!(is_priority_server_line(
        "{\"method\":\"error\",\"params\":{}}"
    ));
    assert!(!is_priority_server_line(
        "{\"method\":\"item/completed\",\"params\":{}}"
    ));
    assert!(!is_priority_server_line("not-json"));
}

#[test]
fn can_submit_queued_turn_only_blocks_on_pending_server_output() {
    let mut deferred = std::collections::VecDeque::new();
    let prefetched = std::collections::VecDeque::new();
    assert!(can_submit_queued_turn(false, &deferred, &prefetched));

    deferred.push_back("tail".to_string());
    assert!(!can_submit_queued_turn(false, &deferred, &prefetched));

    deferred.clear();
    let mut prefetched = std::collections::VecDeque::new();
    prefetched.push_back(UiEvent::Terminal(Event::Resize(80, 24)));
    assert!(can_submit_queued_turn(false, &deferred, &prefetched));

    prefetched.push_back(UiEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Esc,
        KeyModifiers::empty(),
    ))));
    assert!(!can_submit_queued_turn(false, &deferred, &prefetched));

    prefetched.pop_back();
    prefetched.push_back(UiEvent::ServerLine("tail".to_string()));
    assert!(!can_submit_queued_turn(false, &deferred, &prefetched));
    assert!(!can_submit_queued_turn(true, &deferred, &prefetched));
}

#[test]
fn context_usage_label_formats_k_and_percent() {
    let label = context_usage_label(ContextUsage {
        used: 128_000,
        max: 256_000,
    });
    assert_eq!(label, "128k/256k (50%)");
}

#[test]
fn context_label_reserved_cells_uses_fixed_minimum() {
    assert_eq!(
        context_label_reserved_cells(None),
        visual_width("999k/999k (99%)")
    );
}

#[test]
fn context_label_reserved_cells_expands_for_longer_labels() {
    let label = "12345k/12345k (100%)";
    assert_eq!(
        context_label_reserved_cells(Some(label)),
        visual_width(label)
    );
}

#[test]
fn normalize_pasted_text_converts_crlf_and_cr() {
    let text = "a\r\nb\rc";
    assert_eq!(normalize_pasted_text(text), "a\nb\nc");
}

#[test]
fn is_newline_enter_accepts_shift_and_alt() {
    assert!(is_newline_enter(KeyModifiers::SHIFT));
    assert!(is_newline_enter(KeyModifiers::ALT));
    assert!(!is_newline_enter(KeyModifiers::empty()));
}

#[test]
fn is_key_press_like_accepts_repeat() {
    assert!(is_key_press_like(KeyEventKind::Press));
    assert!(is_key_press_like(KeyEventKind::Repeat));
    assert!(!is_key_press_like(KeyEventKind::Release));
}

#[test]
fn pending_claude_exit_plan_accept_uses_approval_path_not_chat_input() {
    let client = ApprovalRespondMock::new();
    let mut app = AppState::new("thread-1".to_string());
    app.set_pending_approval(super::state::PendingApprovalRequest {
        request_id: json!({"backend":"claude","kind":"exitPlanMode","toolUseId":"toolu_1"}),
        method: "claude/exitPlan/requestApproval".to_string(),
        kind: super::state::ApprovalRequestKind::ClaudeExitPlanMode,
        title: "Approve Claude plan".to_string(),
        detail_lines: vec!["Do the work".to_string()],
        requested_permissions: None,
        can_accept_for_session: false,
        can_decline: true,
        can_cancel: false,
    });

    let result = super::input_events::handle_terminal_event(
        &client,
        &mut app,
        Event::Key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::empty())),
        TerminalSize {
            width: 120,
            height: 40,
        },
    );

    assert!(matches!(
        result,
        super::input_events::TerminalEventResult::Continue { needs_draw: true }
    ));
    assert!(app.approval.pending.is_none());
    assert!(app.messages.is_empty());
    assert_eq!(app.input.lines(), [""]);
    let responses = client.responses.lock().expect("responses lock");
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0].1, json!({"decision":"accept"}));
}

#[test]
fn escape_key_disables_ralph_mode() {
    let client = ApprovalRespondMock::new();
    let mut app = AppState::new("thread-1".to_string());
    app.enable_ralph_mode(super::ralph::RalphConfig {
        prompt_path: std::path::PathBuf::from(".agents/ralph-prompt.md"),
        base_prompt: "base".to_string(),
        done_marker: "@@COMPLETE@@".to_string(),
        blocked_marker: "@@BLOCKED@@".to_string(),
        continuation_prompt: "continue".to_string(),
    });
    app.queue_ralph_continuation("continue");

    let result = super::input_events::handle_terminal_event(
        &client,
        &mut app,
        Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty())),
        TerminalSize {
            width: 120,
            height: 40,
        },
    );

    assert!(matches!(
        result,
        super::input_events::TerminalEventResult::Continue { needs_draw: true }
    ));
    assert!(!app.ralph_enabled());
    assert!(!app.has_pending_ralph_continuation());
    assert_eq!(app.status, "ralph off");
}

#[test]
fn submit_rewind_turn_text_forks_codex_thread_before_resubmitting() {
    let fork_response = json!({
        "jsonrpc": "2.0",
        "result": {
            "thread": {
                "id": "thread-fork",
                "turns": [{
                    "items": [
                        {
                            "type": "userMessage",
                            "content": [{
                                "type": "text",
                                "text": "first"
                            }]
                        },
                        {
                            "type": "agentMessage",
                            "text": "reply one"
                        }
                    ]
                }]
            }
        }
    })
    .to_string();
    let client = CodexRewindMock::new(fork_response);
    let mut app = AppState::new("thread-original".to_string());
    let u1 = app.append_message(Role::User, "first");
    app.record_input_history("first", Some(u1));
    let _ = app.append_message(Role::Assistant, "reply one");
    let u2 = app.append_message(Role::User, "second");
    app.record_input_history("second", Some(u2));
    let _ = app.append_message(Role::Assistant, "reply two");
    app.set_input_text("edited second");
    app.enter_rewind_mode();
    let rewind_history_idx = app
        .rewind_selected_history_index()
        .expect("rewind history index");

    super::turn_submit::submit_rewind_turn_text(
        &client,
        &mut app,
        rewind_history_idx,
        "edited second".to_string(),
    );

    assert_eq!(app.thread_id, "thread-fork");
    assert_eq!(app.messages.len(), 2);
    assert_eq!(app.messages[0].text, "first");
    assert_eq!(app.messages[1].text, "reply one");
    assert_eq!(app.input_history_message_indices(), &[Some(0), None]);

    let fork_calls = client.fork_calls.lock().expect("fork calls lock");
    assert_eq!(
        fork_calls.as_slice(),
        &[(
            "thread-original".to_string(),
            RewindForkRequest {
                keep_turns: 1,
                drop_turns: 1,
            },
        )]
    );

    let turn_start_calls = client
        .turn_start_calls
        .lock()
        .expect("turn start calls lock");
    assert_eq!(turn_start_calls.len(), 1);
    assert_eq!(turn_start_calls[0]["threadId"], "thread-fork");
    assert_eq!(turn_start_calls[0]["input"][0]["text"], "edited second");
}

#[test]
fn decide_mouse_drag_mode_prefers_scroll_for_vertical_swipe() {
    assert_eq!(
        decide_mouse_drag_mode(10, 10, 10, 14),
        MouseDragMode::Scroll
    );
    assert_eq!(
        decide_mouse_drag_mode(10, 10, 12, 14),
        MouseDragMode::Select
    );
    assert_eq!(
        decide_mouse_drag_mode(10, 10, 10, 11),
        MouseDragMode::Select
    );
    assert_eq!(
        decide_mouse_drag_mode(10, 10, 10, 10),
        MouseDragMode::Undecided
    );
}

#[test]
fn parse_mobile_mouse_coords_accepts_plain_and_sgr_fragments() {
    assert_eq!(parse_mobile_mouse_coords("76;46"), Some((76, 46)));
    assert_eq!(
        parse_mobile_mouse_coords("\u{1b}[<64;76;46M"),
        Some((76, 46))
    );
    assert_eq!(parse_mobile_mouse_coords("hello"), None);
}

#[test]
fn consume_mobile_mouse_char_does_not_swallow_plain_digits() {
    let mut app = AppState::new("thread-1".to_string());
    assert!(matches!(
        consume_mobile_mouse_char(&mut app, '2'),
        MobileMouseConsume::PassThrough
    ));
    assert!(app.viewport.mobile_mouse_buffer.is_empty());
}

#[test]
fn consume_mobile_mouse_char_requires_prefix_to_activate() {
    let mut app = AppState::new("thread-1".to_string());
    assert!(matches!(
        consume_mobile_mouse_char(&mut app, '<'),
        MobileMouseConsume::Consumed
    ));
    assert!(matches!(
        consume_mobile_mouse_char(&mut app, '7'),
        MobileMouseConsume::Consumed
    ));
    assert!(matches!(
        consume_mobile_mouse_char(&mut app, '6'),
        MobileMouseConsume::Consumed
    ));
    assert!(matches!(
        consume_mobile_mouse_char(&mut app, ';'),
        MobileMouseConsume::Consumed
    ));
}

#[test]
fn consume_mobile_mouse_char_accepts_csi_bracket_prefix() {
    let mut app = AppState::new("thread-1".to_string());
    app.viewport.scroll_top = 12;
    app.viewport.mobile_mouse_last_y = Some(40);

    for ch in ['[', '<', '6', '4', ';', '7', '6', ';', '4', '6', 'M'] {
        assert!(matches!(
            consume_mobile_mouse_char(&mut app, ch),
            MobileMouseConsume::Consumed
        ));
    }
    assert_eq!(app.viewport.scroll_top, 18);
    assert!(app.viewport.mobile_mouse_buffer.is_empty());
}

#[test]
fn mobile_mouse_key_candidate_accepts_alt_prefixed_csi_chars() {
    let app = AppState::new("thread-1".to_string());
    assert!(is_mobile_mouse_key_candidate(
        &app,
        KeyCode::Char('['),
        KeyModifiers::ALT
    ));
    assert!(is_mobile_mouse_key_candidate(
        &app,
        KeyCode::Char('7'),
        KeyModifiers::ALT
    ));
    assert!(!is_mobile_mouse_key_candidate(
        &app,
        KeyCode::Char('7'),
        KeyModifiers::empty()
    ));
}

#[test]
fn mobile_mouse_key_candidate_accepts_plain_coords_when_pending() {
    let mut app = AppState::new("thread-1".to_string());
    app.viewport.mobile_plain_pending_coords = true;
    assert!(is_mobile_mouse_key_candidate(
        &app,
        KeyCode::Char('7'),
        KeyModifiers::empty()
    ));
    assert!(is_mobile_mouse_key_candidate(
        &app,
        KeyCode::Char(';'),
        KeyModifiers::empty()
    ));
    assert!(!is_mobile_mouse_key_candidate(
        &app,
        KeyCode::Char('a'),
        KeyModifiers::empty()
    ));
}

#[test]
fn consume_mobile_mouse_char_plain_pending_pair_applies_scroll() {
    let mut app = AppState::new("thread-1".to_string());
    app.viewport.mobile_plain_pending_coords = true;
    app.viewport.scroll_top = 20;
    app.viewport.mobile_mouse_last_y = Some(50);

    for ch in ['6', '6', ';', '5', '2'] {
        assert!(matches!(
            consume_mobile_mouse_char(&mut app, ch),
            MobileMouseConsume::Consumed
        ));
    }

    assert_eq!(app.viewport.scroll_top, 23);
    assert!(!app.viewport.mobile_plain_pending_coords);
    assert!(app.viewport.mobile_mouse_buffer.is_empty());
}

#[test]
fn consume_mobile_mouse_char_plain_pending_repeated_pair_reuses_direction() {
    let mut app = AppState::new("thread-1".to_string());
    app.viewport.scroll_top = 20;
    app.viewport.mobile_mouse_last_y = Some(50);

    app.viewport.mobile_plain_pending_coords = true;
    for ch in ['6', '6', ';', '5', '2'] {
        let _ = consume_mobile_mouse_char(&mut app, ch);
    }
    assert_eq!(app.viewport.scroll_top, 23);

    app.viewport.mobile_plain_pending_coords = true;
    for ch in ['6', '6', ';', '5', '2'] {
        let _ = consume_mobile_mouse_char(&mut app, ch);
    }
    assert_eq!(app.viewport.scroll_top, 26);
}

#[test]
fn consume_mobile_mouse_char_plain_pending_new_gesture_keeps_prior_direction() {
    let mut app = AppState::new("thread-1".to_string());
    app.viewport.scroll_top = 20;
    app.viewport.mobile_mouse_last_y = Some(50);
    app.viewport.mobile_plain_last_direction = 1;
    app.viewport.mobile_plain_new_gesture = true;
    app.viewport.mobile_plain_pending_coords = true;

    for ch in ['6', '4', ';', '4', '7'] {
        let _ = consume_mobile_mouse_char(&mut app, ch);
    }

    assert_eq!(app.viewport.scroll_top, 23);
    assert_eq!(app.viewport.mobile_plain_last_direction, 1);
}

#[test]
fn parse_repeated_plain_mobile_pair_accepts_concatenated_repetition() {
    assert_eq!(
        parse_repeated_plain_mobile_pair("75;4375;4375;43"),
        Some((75, 43))
    );
    assert_eq!(
        parse_repeated_plain_mobile_pair("71;5371;5371;53"),
        Some((71, 53))
    );
    assert_eq!(parse_repeated_plain_mobile_pair("75;43"), None);
}

#[test]
fn consume_mobile_mouse_char_applies_scroll_on_terminator() {
    let mut app = AppState::new("thread-1".to_string());
    app.viewport.scroll_top = 10;
    app.viewport.mobile_mouse_last_y = Some(40);

    for ch in ['<', '6', '4', ';', '7', '6', ';', '4', '6'] {
        assert!(matches!(
            consume_mobile_mouse_char(&mut app, ch),
            MobileMouseConsume::Consumed
        ));
    }
    assert!(matches!(
        consume_mobile_mouse_char(&mut app, 'M'),
        MobileMouseConsume::Consumed
    ));
    assert_eq!(app.viewport.scroll_top, 16);
    assert!(app.viewport.mobile_mouse_buffer.is_empty());
}

#[test]
fn apply_mobile_mouse_scroll_uses_natural_touch_direction() {
    let mut app = AppState::new("thread-1".to_string());
    app.viewport.scroll_top = 20;
    app.viewport.mobile_mouse_last_y = Some(40);

    apply_mobile_mouse_scroll(&mut app, 44);
    assert_eq!(app.viewport.scroll_top, 24);

    apply_mobile_mouse_scroll(&mut app, 42);
    assert_eq!(app.viewport.scroll_top, 22);
}

#[test]
fn apply_mobile_mouse_scroll_honors_invert_toggle() {
    let mut app = AppState::new("thread-1".to_string());
    app.viewport.scroll_inverted = true;
    app.viewport.scroll_top = 20;
    app.viewport.mobile_mouse_last_y = Some(40);

    apply_mobile_mouse_scroll(&mut app, 44);
    assert_eq!(app.viewport.scroll_top, 16);

    apply_mobile_mouse_scroll(&mut app, 42);
    assert_eq!(app.viewport.scroll_top, 18);
}

#[test]
fn kitt_head_index_bounces_across_separator() {
    let seq: Vec<usize> = (0..9).map(|tick| kitt_head_index(5, tick)).collect();
    assert_eq!(seq, vec![0, 1, 2, 3, 4, 3, 2, 1, 0]);
    assert_eq!(kitt_head_index(1, 42), 0);
}

#[test]

fn duration_samples_tracks_percentiles_and_window() {
    let mut samples = DurationSamples::new(3);
    samples.push(Duration::from_micros(1_000));
    samples.push(Duration::from_micros(2_000));
    samples.push(Duration::from_micros(3_000));
    samples.push(Duration::from_micros(4_000));

    // Window size is 3, so first sample is dropped.
    assert_eq!(samples.values_us.len(), 3);
    assert_eq!(samples.percentile_us(0.50), Some(3_000));
    assert_eq!(samples.percentile_us(0.95), Some(4_000));
    assert_eq!(samples.avg_ms(), 2.50);
    assert_eq!(samples.max_ms(), 4.0);
}

#[test]
fn perf_metrics_overlay_lines_include_latency_rows() {
    let mut perf = PerfMetrics::new();
    perf.poll_wait.push(Duration::from_micros(500));
    perf.event_handle.push(Duration::from_micros(700));
    perf.draw.push(Duration::from_micros(900));
    perf.key_interval.push(Duration::from_micros(1_100));
    perf.repeat_interval.push(Duration::from_micros(1_300));
    perf.press_to_first_repeat
        .push(Duration::from_micros(1_500));
    perf.release_to_next_key.push(Duration::from_micros(1_700));

    let lines = perf.overlay_lines();
    assert!(lines.iter().any(|line| line.contains("poll wait")));
    assert!(lines.iter().any(|line| line.contains("event handle")));
    assert!(lines.iter().any(|line| line.contains("draw")));
    assert!(lines.iter().any(|line| line.contains("key interval")));
    assert!(lines.iter().any(|line| line.contains("repeat intvl")));
    assert!(lines.iter().any(|line| line.contains("press->repeat")));
    assert!(lines.iter().any(|line| line.contains("release->key")));
}

#[test]
fn perf_metrics_tracks_repeat_transition_buckets() {
    let mut perf = PerfMetrics::new();
    perf.mark_key_kind(KeyEventKind::Press);
    perf.mark_key_kind(KeyEventKind::Repeat);
    perf.mark_key_kind(KeyEventKind::Repeat);
    perf.mark_key_kind(KeyEventKind::Release);
    perf.mark_key_kind(KeyEventKind::Press);

    assert_eq!(perf.key_press_events, 2);
    assert_eq!(perf.key_repeat_events, 2);
    assert_eq!(perf.key_release_events, 1);
    assert_eq!(perf.press_to_first_repeat.values_us.len(), 1);
    assert_eq!(perf.repeat_interval.values_us.len(), 1);
    assert_eq!(perf.release_to_next_key.values_us.len(), 1);
}
