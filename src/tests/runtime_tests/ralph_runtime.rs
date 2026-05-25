// Tests for ralph-mode runtime state transitions and persistence.

#[test]
fn resolve_initial_runtime_settings_falls_back_to_persisted_defaults() {
    let runtime = crate::protocol_params::ThreadRuntimeSettings {
        model: None,
        effort: None,
        summary: None,
    };
    let defaults = RuntimeDefaults {
        model: Some("gpt-5.4".to_string()),
        effort: Some("high".to_string()),
        summary: Some("concise".to_string()),
    };

    let resolved = resolve_initial_runtime_settings(runtime, &defaults, defaults.summary.clone());

    assert_eq!(resolved.model.as_deref(), Some("gpt-5.4"));
    assert_eq!(resolved.effort.as_deref(), Some("high"));
    assert_eq!(resolved.summary.as_deref(), Some("concise"));
}

#[test]
fn apply_model_settings_returns_defaults_for_persistence() {
    let mut app = AppState::new("thread-1".to_string());
    app.runtime.model_settings_model_input = "gpt-5.4".to_string();
    app.runtime.model_settings_effort_options = super::state::DEFAULT_EFFORT_OPTIONS
        .iter()
        .map(|value| (*value).to_string())
        .collect();
    app.runtime.model_settings_effort_index = 4;
    app.runtime.model_settings_summary_options = super::state::DEFAULT_SUMMARY_OPTIONS
        .iter()
        .map(|value| (*value).to_string())
        .collect();
    app.runtime.model_settings_summary_index = 1;
    app.runtime.show_model_settings = true;

    let defaults = app.apply_model_settings();

    assert_eq!(
        defaults,
        RuntimeDefaults {
            model: Some("gpt-5.4".to_string()),
            effort: Some("high".to_string()),
            summary: Some("concise".to_string()),
        }
    );
    assert_eq!(
        app.take_pending_runtime_settings(),
        (
            Some("gpt-5.4".to_string()),
            Some("high".to_string()),
            Some("concise".to_string())
        )
    );
    assert!(!app.runtime.show_model_settings);
}

#[test]
fn apply_model_settings_omits_summary_when_backend_does_not_support_it() {
    let mut app = AppState::new("thread-1".to_string());
    app.set_runtime_capabilities(true, false);
    app.runtime.model_settings_model_input = "claude-opus-4-6".to_string();
    app.runtime.model_settings_effort_options = vec![
        "low".to_string(),
        "medium".to_string(),
        "high".to_string(),
        "max".to_string(),
    ];
    app.runtime.model_settings_effort_index = 2;
    app.runtime.model_settings_summary_options = super::state::DEFAULT_SUMMARY_OPTIONS
        .iter()
        .map(|value| (*value).to_string())
        .collect();
    app.runtime.model_settings_summary_index = 1;
    app.runtime.show_model_settings = true;

    let defaults = app.apply_model_settings();

    assert_eq!(
        defaults,
        RuntimeDefaults {
            model: Some("claude-opus-4-6".to_string()),
            effort: Some("high".to_string()),
            summary: None,
        }
    );
    assert_eq!(
        app.take_pending_runtime_settings(),
        (
            Some("claude-opus-4-6".to_string()),
            Some("high".to_string()),
            None
        )
    );
}

#[test]
fn detect_turn_markers_matches_trimmed_commentary_marker_lines() {
    let messages = vec![
        Message {
            role: Role::User,
            text: "@@BLOCKED@@".to_string(),
            kind: MessageKind::Plain,
            file_path: None,
        },
        Message {
            role: Role::Commentary,
            text: "working...\n  @@BLOCKED@@ \nnext".to_string(),
            kind: MessageKind::Plain,
            file_path: None,
        },
        Message {
            role: Role::Assistant,
            text: "@@COMPLETE@@".to_string(),
            kind: MessageKind::Plain,
            file_path: None,
        },
    ];

    let markers = super::ralph::detect_turn_markers(&messages, 0, "@@COMPLETE@@", "@@BLOCKED@@");
    assert!(markers.blocked);
    assert!(markers.completed);
}

#[test]
fn detect_turn_markers_matches_inline_marker_tokens() {
    let messages = vec![Message {
        role: Role::Assistant,
        text: "@@COMPLETE@@ @@COMPLETE_SUMMARY_START@@ summary".to_string(),
        kind: MessageKind::Plain,
        file_path: None,
    }];

    let markers = super::ralph::detect_turn_markers(&messages, 0, "@@COMPLETE@@", "@@BLOCKED@@");
    assert!(markers.completed);
    assert!(!markers.blocked);
}

#[test]
fn ralph_turn_completion_queues_continuation_when_not_blocked_or_complete() {
    let mut app = AppState::new("thread-1".to_string());
    app.enable_ralph_mode(super::ralph::RalphConfig {
        prompt_path: std::path::PathBuf::from(".agents/ralph-prompt.md"),
        base_prompt: "base".to_string(),
        done_marker: "@@COMPLETE@@".to_string(),
        blocked_marker: "@@BLOCKED@@".to_string(),
        continuation_prompt: "continue".to_string(),
    });
    app.turn_start_message_idx = Some(0);
    app.append_message(Role::Assistant, "still working");

    app.handle_ralph_turn_completed(false);

    assert!(app.has_pending_ralph_continuation());
    assert!(app.dequeue_turn_input(std::time::Instant::now()).is_none());
}

#[test]
fn ralph_turn_completion_disables_ralph_mode_on_blocked_marker() {
    let mut app = AppState::new("thread-1".to_string());
    app.enable_ralph_mode(super::ralph::RalphConfig {
        prompt_path: std::path::PathBuf::from(".agents/ralph-prompt.md"),
        base_prompt: "base".to_string(),
        done_marker: "@@COMPLETE@@".to_string(),
        blocked_marker: "@@BLOCKED@@".to_string(),
        continuation_prompt: "continue".to_string(),
    });
    app.turn_start_message_idx = Some(0);
    app.append_message(Role::Assistant, "@@BLOCKED@@");

    app.handle_ralph_turn_completed(false);

    assert!(app.queued_turn_inputs_is_empty());
    assert!(!app.ralph_enabled());
}

#[test]
fn commentary_blocked_marker_disables_ralph_mode_immediately() {
    let mut app = AppState::new("thread-1".to_string());
    app.enable_ralph_mode(super::ralph::RalphConfig {
        prompt_path: std::path::PathBuf::from(".agents/ralph-prompt.md"),
        base_prompt: "base".to_string(),
        done_marker: "@@COMPLETE@@".to_string(),
        blocked_marker: "@@BLOCKED@@".to_string(),
        continuation_prompt: "continue".to_string(),
    });
    app.turn_start_message_idx = Some(0);
    app.append_message(Role::Commentary, "@@BLOCKED@@");

    app.maybe_disable_ralph_on_blocked_marker();

    assert!(!app.ralph_enabled());
    assert!(app.messages.last().is_some_and(
        |msg| msg.role == Role::System && msg.text == "Ralph blocked: waiting for input"
    ));
}

#[test]
fn ralph_turn_completion_disables_ralph_mode() {
    let mut app = AppState::new("thread-1".to_string());
    app.enable_ralph_mode(super::ralph::RalphConfig {
        prompt_path: std::path::PathBuf::from(".agents/ralph-prompt.md"),
        base_prompt: "base".to_string(),
        done_marker: "@@COMPLETE@@".to_string(),
        blocked_marker: "@@BLOCKED@@".to_string(),
        continuation_prompt: "continue".to_string(),
    });
    app.turn_start_message_idx = Some(0);
    app.append_message(
        Role::Assistant,
        "@@COMPLETE@@ @@COMPLETE_SUMMARY_START@@".to_string(),
    );

    app.handle_ralph_turn_completed(false);

    assert!(!app.ralph_enabled());
    assert!(app.queued_turn_inputs_is_empty());
}

#[test]
fn ralph_interrupted_turn_does_not_queue_continuation() {
    let mut app = AppState::new("thread-1".to_string());
    app.enable_ralph_mode(super::ralph::RalphConfig {
        prompt_path: std::path::PathBuf::from(".agents/ralph-prompt.md"),
        base_prompt: "base".to_string(),
        done_marker: "@@COMPLETE@@".to_string(),
        blocked_marker: "@@BLOCKED@@".to_string(),
        continuation_prompt: "continue".to_string(),
    });
    app.turn_start_message_idx = Some(0);
    app.append_message(Role::Assistant, "still working");

    app.handle_ralph_turn_completed(true);

    assert!(app.queued_turn_inputs_is_empty());
    assert!(app.ralph_enabled());
    assert!(!app.ralph_waiting_for_user());
}

#[test]
fn request_ralph_toggle_enables_and_disables_when_idle() {
    let mut app = AppState::new("thread-1".to_string());
    let tmp = std::env::temp_dir().join(format!(
        "carlos-ralph-toggle-{}.md",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("duration")
            .as_nanos()
    ));
    std::fs::write(&tmp, "auto prompt").expect("write prompt");
    app.configure_ralph_options(
        std::env::current_dir().expect("cwd"),
        Some(tmp.to_string_lossy().to_string()),
        None,
        None,
    );

    app.request_ralph_toggle().expect("toggle on");
    assert!(app.ralph_enabled());
    assert!(app.dequeue_turn_input(std::time::Instant::now()).is_some());

    app.request_ralph_toggle().expect("toggle off");
    assert!(!app.ralph_enabled());
    assert!(app.queued_turn_inputs_is_empty());

    let _ = std::fs::remove_file(tmp);
}

#[test]
fn request_ralph_toggle_defers_while_turn_active() {
    let mut app = AppState::new("thread-1".to_string());
    app.active_turn_id = Some("turn-1".to_string());

    app.request_ralph_toggle().expect("defer");
    assert!(app.ralph_toggle_pending());

    app.request_ralph_toggle().expect("cancel");
    assert!(!app.ralph_toggle_pending());
}

#[test]
fn pending_ralph_continuation_becomes_ready_after_deadline() {
    let mut app = AppState::new("thread-1".to_string());
    app.queue_ralph_continuation("continue");
    let deadline = app
        .ralph_pending_continuation_deadline()
        .expect("continuation deadline");

    assert!(app
        .dequeue_turn_input(deadline - std::time::Duration::from_millis(1))
        .is_none());
    let queued = app
        .dequeue_turn_input(deadline)
        .expect("queued continuation");
    assert_eq!(queued.text, "continue");
    assert!(!queued.record_input_history);
    assert!(!app.has_pending_ralph_continuation());
}

#[test]
fn queued_user_turns_record_history_but_ralph_turns_do_not() {
    let mut app = AppState::new("thread-1".to_string());
    app.queue_turn_input("follow up");
    let user_turn = app
        .dequeue_turn_input(std::time::Instant::now())
        .expect("queued user turn");
    assert_eq!(user_turn.text, "follow up");
    assert!(user_turn.record_input_history);

    app.queue_ralph_continuation("continue");
    let deadline = app
        .ralph_pending_continuation_deadline()
        .expect("continuation deadline");
    let ralph_turn = app.dequeue_turn_input(deadline).expect("queued Ralph turn");
    assert_eq!(ralph_turn.text, "continue");
    assert!(!ralph_turn.record_input_history);
}
