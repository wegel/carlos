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
fn enter_stops_dictation_recording_to_transcribing() {
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
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())),
        size,
    );

    assert!(matches!(
        app.dictation_phase(),
        DictationPhase::Transcribing { .. }
    ));
}

#[test]
fn f9_toggles_dictation_endpoint_mode() {
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
        Event::Key(KeyEvent::new(KeyCode::F(9), KeyModifiers::empty())),
        size,
    );
    assert_eq!(app.status, "dictation endpoint: manual (Enter to stop)");

    app.start_dictation_recording();
    assert_eq!(
        app.dictation_status_label(),
        Some("DICTATING MANUAL [English]".to_string())
    );

    handle_terminal_event(
        &client,
        &mut app,
        Event::Key(KeyEvent::new(KeyCode::F(9), KeyModifiers::empty())),
        size,
    );
    assert_eq!(app.status, "dictation endpoint: auto");
    assert_eq!(
        app.dictation_status_label(),
        Some("DICTATING AUTO [English]".to_string())
    );
}

#[test]
fn model_settings_can_toggle_dictation_endpoint_mode() {
    let client = ApprovalRespondMock::new();
    let mut app = AppState::new("thread-1".to_string());
    app.configure_dictation(usable_dictation_profile());
    let size = TerminalSize {
        width: 100,
        height: 30,
    };

    app.open_model_settings();
    app.model_settings_move_field(true);
    app.model_settings_move_field(true);
    app.model_settings_move_field(true);
    handle_terminal_event(
        &client,
        &mut app,
        Event::Key(KeyEvent::new(KeyCode::Right, KeyModifiers::empty())),
        size,
    );

    assert!(app.runtime.show_model_settings);
    assert_eq!(app.status, "dictation endpoint: manual (Enter to stop)");
    assert_eq!(
        app.dictation_status_label(),
        None,
        "toggling endpoint mode should not start recording"
    );

    app.start_dictation_recording();
    assert_eq!(
        app.dictation_status_label(),
        Some("DICTATING MANUAL [English]".to_string())
    );
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
        crate::dictation::events::DictationEvent::CaptureAutoStopped(Arc::new(vec![0.1, 0.2, 0.3])),
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
