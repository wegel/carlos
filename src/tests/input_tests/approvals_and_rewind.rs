// Tests for approval prompts and rewind-mode keyboard handling.

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
        Some("DICTATING AUTO [French]".to_string())
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
        waiting_marker: "@@AWAITING_HUMAN@@".to_string(),
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

fn build_rewind_fork_response() -> String {
    json!({
        "jsonrpc": "2.0",
        "result": {
            "thread": {
                "id": "thread-fork",
                "turns": [{
                    "items": [
                        {
                            "type": "userMessage",
                            "content": [{ "type": "text", "text": "first" }]
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
    .to_string()
}

fn build_two_turn_rewind_app() -> AppState {
    let mut app = AppState::new("thread-original".to_string());
    let u1 = app.append_message(Role::User, "first");
    app.record_input_history("first", Some(u1));
    let _ = app.append_message(Role::Assistant, "reply one");
    let u2 = app.append_message(Role::User, "second");
    app.record_input_history("second", Some(u2));
    let _ = app.append_message(Role::Assistant, "reply two");
    app.set_input_text("edited second");
    app.enter_rewind_mode();
    app
}

#[test]
fn submit_rewind_turn_text_forks_codex_thread_before_resubmitting() {
    let client = CodexRewindMock::new(build_rewind_fork_response());
    let mut app = build_two_turn_rewind_app();
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
