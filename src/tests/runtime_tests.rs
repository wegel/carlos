use super::*;


#[test]
fn osc52_wrap_detects_tmux_and_screen() {
    assert_eq!(
        detect_osc52_wrap(Some("/tmp/tmux-1000/default,123,0"), Some("xterm-256color")),
        Osc52Wrap::Tmux
    );
    assert_eq!(
        detect_osc52_wrap(None, Some("screen-256color")),
        Osc52Wrap::Screen
    );
    assert_eq!(
        detect_osc52_wrap(None, Some("xterm-256color")),
        Osc52Wrap::None
    );
}

#[test]
fn osc52_tmux_sequence_uses_passthrough_and_escaped_esc() {
    let encoded = "YQ==";
    let seqs = osc52_sequences_for_env(encoded, Some("1"), Some("xterm-256color"));
    let first = &seqs[0];
    assert!(first.starts_with("\x1bPtmux;"));
    assert!(first.contains("\x1b\x1b]52;c;YQ=="));
    assert!(first.ends_with("\x1b\\"));
}

#[test]
fn osc52_screen_sequence_uses_dcs_wrapper() {
    let encoded = "YQ==";
    let seqs = osc52_sequences_for_env(encoded, None, Some("screen-256color"));
    let first = &seqs[0];
    assert!(first.starts_with("\x1bP\x1b]52;c;YQ=="));
    assert!(first.ends_with("\x1b\\"));
}

#[test]
fn osc52_generates_both_clipboard_targets() {
    let seqs = osc52_sequences_for_env("YQ==", None, Some("xterm-256color"));
    assert!(seqs.iter().any(|s| s.contains("]52;c;YQ==")));
    assert!(seqs.iter().any(|s| s.contains("]52;p;YQ==")));
}

#[test]
fn ssh_detection_works() {
    assert!(is_ssh_session(Some("/dev/pts/3"), None, None));
    assert!(is_ssh_session(None, Some("1.2.3.4 22 5.6.7.8 54321"), None));
    assert!(is_ssh_session(None, None, Some("1.2.3.4 54321 22")));
    assert!(!is_ssh_session(None, None, None));
}

#[test]
fn parse_cli_args_supports_ralph_resume_and_markers() {
    let args = vec![
        "resume".to_string(),
        "session-123".to_string(),
        "--ralph-prompt".to_string(),
        "custom/prompt.md".to_string(),
        "--ralph-done-marker".to_string(),
        "DONE".to_string(),
        "--ralph-blocked-marker".to_string(),
        "BLOCKED".to_string(),
    ];
    let parsed = parse_cli_args(args).expect("parse");

    assert!(parsed.mode_resume);
    assert_eq!(parsed.resume_id.as_deref(), Some("session-123"));
    assert_eq!(
        parsed.ralph_prompt_path.as_deref(),
        Some("custom/prompt.md")
    );
    assert_eq!(parsed.ralph_done_marker.as_deref(), Some("DONE"));
    assert_eq!(parsed.ralph_blocked_marker.as_deref(), Some("BLOCKED"));
}

#[test]
fn parse_cli_args_supports_continue_mode() {
    let args = vec!["continue".to_string()];
    let parsed = parse_cli_args(args).expect("parse");

    assert!(parsed.mode_continue);
    assert!(!parsed.mode_resume);
    assert!(!parsed.mode_perf_session);
    assert_eq!(parsed.resume_id, None);
}

#[test]
fn parse_cli_args_supports_perf_session_mode() {
    let args = vec![
        "perf-session".to_string(),
        "/tmp/session.jsonl".to_string(),
        "--width".to_string(),
        "200".to_string(),
        "--height".to_string(),
        "60".to_string(),
    ];
    let parsed = parse_cli_args(args).expect("parse");

    assert!(parsed.mode_perf_session);
    assert!(!parsed.mode_resume);
    assert_eq!(
        parsed.perf_session_path.as_deref(),
        Some("/tmp/session.jsonl")
    );
    assert_eq!(parsed.perf_width, 200);
    assert_eq!(parsed.perf_height, 60);
}

#[test]
fn parse_cli_args_supports_synthetic_perf_session_mode() {
    let args = vec![
        "perf-session".to_string(),
        "--synthetic".to_string(),
        "--turns".to_string(),
        "250".to_string(),
        "--seed".to_string(),
        "17".to_string(),
        "--tool-lines".to_string(),
        "40".to_string(),
    ];
    let parsed = parse_cli_args(args).expect("parse");

    assert!(parsed.mode_perf_session);
    assert!(parsed.perf_synthetic);
    assert_eq!(parsed.perf_session_path, None);
    assert_eq!(parsed.perf_turns, 250);
    assert_eq!(parsed.perf_seed, 17);
    assert_eq!(parsed.perf_tool_lines, 40);
}

#[test]
fn synthetic_perf_messages_are_reproducible() {
    let spec = perf_session::SyntheticPerfSpec {
        seed: 7,
        turns: 12,
        tool_output_lines: 8,
    };
    let left = perf_session::build_synthetic_perf_messages(spec);
    let right = perf_session::build_synthetic_perf_messages(spec);

    assert_eq!(left.len(), spec.turns * 6);
    assert_eq!(left.len(), right.len());
    for (lhs, rhs) in left.iter().zip(right.iter()) {
        assert_eq!(lhs.role, rhs.role);
        assert_eq!(lhs.kind, rhs.kind);
        assert_eq!(lhs.text, rhs.text);
        assert_eq!(lhs.file_path, rhs.file_path);
    }
}

#[test]
fn synthetic_perf_messages_change_with_seed() {
    let base = perf_session::build_synthetic_perf_messages(perf_session::SyntheticPerfSpec {
        seed: 7,
        turns: 4,
        tool_output_lines: 4,
    });
    let changed = perf_session::build_synthetic_perf_messages(perf_session::SyntheticPerfSpec {
        seed: 8,
        turns: 4,
        tool_output_lines: 4,
    });

    assert_eq!(base.len(), changed.len());
    assert!(base
        .iter()
        .zip(changed.iter())
        .any(|(lhs, rhs)| lhs.text != rhs.text));
}

#[test]
fn parse_thread_runtime_settings_reads_model_and_effort() {
    let response = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "thread": { "id": "thread-1" },
            "model": "gpt-5-codex",
            "reasoningEffort": "high",
            "summary": "auto"
        }
    })
    .to_string();

    let settings = parse_thread_runtime_settings(&response).expect("settings");
    assert_eq!(settings.model.as_deref(), Some("gpt-5-codex"));
    assert_eq!(settings.effort.as_deref(), Some("high"));
    assert_eq!(settings.summary.as_deref(), Some("auto"));
}

#[test]
fn parse_thread_runtime_settings_accepts_effort_alias() {
    let response = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "thread": { "id": "thread-1" },
            "model": "gpt-5.4",
            "effort": "medium"
        }
    })
    .to_string();

    let settings = parse_thread_runtime_settings(&response).expect("settings");
    assert_eq!(settings.model.as_deref(), Some("gpt-5.4"));
    assert_eq!(settings.effort.as_deref(), Some("medium"));
}

#[test]
fn params_turn_start_includes_model_effort_and_summary_when_set() {
    let params = params_turn_start(
        "thread-1",
        "hello",
        Some("gpt-5"),
        Some("high"),
        Some("auto"),
    );
    assert_eq!(params["threadId"], "thread-1");
    assert_eq!(params["model"], "gpt-5");
    assert_eq!(params["effort"], "high");
    assert_eq!(params["summary"], "auto");
}

#[test]
fn params_turn_start_omits_model_effort_and_summary_when_missing() {
    let params = params_turn_start("thread-1", "hello", None, None, None);
    assert!(params.get("model").is_none());
    assert!(params.get("effort").is_none());
    assert!(params.get("summary").is_none());
}

#[test]
fn params_thread_archive_includes_thread_id() {
    let params = params_thread_archive("thread-123");

    assert_eq!(params["threadId"], "thread-123");
}

#[test]
fn runtime_settings_label_shows_pending_summary_override() {
    let mut app = AppState::new("thread-1".to_string());
    app.set_runtime_settings(Some("gpt-5".to_string()), Some("high".to_string()), None);
    app.apply_default_reasoning_summary(Some("auto".to_string()));

    assert_eq!(app.runtime_settings_label(), "gpt-5/high/auto*");
}

#[test]
fn next_turn_runtime_settings_falls_back_to_current_values() {
    let mut app = AppState::new("thread-1".to_string());
    app.set_runtime_settings(
        Some("gpt-5.4".to_string()),
        Some("high".to_string()),
        Some("concise".to_string()),
    );

    assert_eq!(
        app.next_turn_runtime_settings(),
        (
            Some("gpt-5.4".to_string()),
            Some("high".to_string()),
            Some("concise".to_string())
        )
    );
}

#[test]
fn next_turn_runtime_settings_prefers_pending_over_current_values() {
    let mut app = AppState::new("thread-1".to_string());
    app.set_runtime_settings(
        Some("gpt-5.4".to_string()),
        Some("high".to_string()),
        Some("concise".to_string()),
    );
    app.queue_runtime_settings(
        Some("gpt-5.4-mini".to_string()),
        Some("medium".to_string()),
        Some("auto".to_string()),
    );

    assert_eq!(
        app.next_turn_runtime_settings(),
        (
            Some("gpt-5.4-mini".to_string()),
            Some("medium".to_string()),
            Some("auto".to_string())
        )
    );
}

#[test]
fn runtime_defaults_round_trip_json_file() {
    let path = std::env::temp_dir().join(format!(
        "carlos-runtime-defaults-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let defaults = RuntimeDefaults {
        model: Some("gpt-5.4".to_string()),
        effort: Some("high".to_string()),
        summary: Some("concise".to_string()),
    };

    persist_runtime_defaults_to(&path, &defaults).expect("persist");
    let loaded = load_runtime_defaults_from(&path);
    let _ = std::fs::remove_file(&path);

    assert_eq!(loaded, defaults);
}

#[test]
fn resolve_initial_runtime_settings_falls_back_to_persisted_defaults() {
    let runtime = crate::protocol::ThreadRuntimeSettings {
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
    let queued = app.dequeue_turn_input(deadline).expect("queued continuation");
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

#[test]
fn delayed_ralph_continuation_does_not_block_ready_user_turns() {
    let mut app = AppState::new("thread-1".to_string());
    app.queue_turn_input("first");
    app.queue_ralph_continuation("continue");
    app.queue_turn_input("second");

    let first = app
        .dequeue_turn_input(std::time::Instant::now())
        .expect("first queued turn");
    assert_eq!(first.text, "first");

    let second = app
        .dequeue_turn_input(std::time::Instant::now())
        .expect("second queued turn");
    assert_eq!(second.text, "second");

    let deadline = app
        .ralph_pending_continuation_deadline()
        .expect("continuation deadline");
    let continuation = app.dequeue_turn_input(deadline).expect("queued continuation");
    assert_eq!(continuation.text, "continue");
}

#[test]
fn parse_thread_list_reads_session_summary_fields() {
    let response = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "data": [
                {
                    "id": "thread-123",
                    "name": "Parser follow-up",
                    "preview": "Fix the failing parser test",
                    "cwd": "/repo",
                    "createdAt": 1_731_100_000,
                    "updatedAt": 1_731_200_430
                },
                {
                    "preview": "missing id should be ignored",
                    "cwd": "/repo",
                    "updatedAt": 1
                }
            ]
        }
    })
    .to_string();

    let threads = parse_thread_list(&response).expect("parse thread list");
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0].id, "thread-123");
    assert_eq!(threads[0].name.as_deref(), Some("Parser follow-up"));
    assert_eq!(threads[0].preview, "Fix the failing parser test");
    assert_eq!(threads[0].cwd, "/repo");
    assert_eq!(threads[0].created_at, 1_731_100_000);
    assert_eq!(threads[0].updated_at, 1_731_200_430);
}

#[test]
fn resume_hint_formats_resume_command() {
    assert_eq!(
        resume_hint("xxxx-xxxx"),
        "to resume this session use:\ncarlos resume xxxx-xxxx"
    );
}

#[test]
fn styled_resume_hint_colors_only_command_line() {
    assert_eq!(
        styled_resume_hint("xxxx-xxxx"),
        "to resume this session use:\n\x1b[94mcarlos resume xxxx-xxxx\x1b[0m"
    );
}

#[test]
fn sort_threads_for_picker_prefers_most_recent_update_first() {
    let threads = vec![
        ThreadSummary {
            id: "older".to_string(),
            name: None,
            preview: "older".to_string(),
            cwd: "/repo".to_string(),
            created_at: 10,
            updated_at: 20,
        },
        ThreadSummary {
            id: "newer".to_string(),
            name: None,
            preview: "newer".to_string(),
            cwd: "/repo".to_string(),
            created_at: 11,
            updated_at: 30,
        },
        ThreadSummary {
            id: "same-update-later-created".to_string(),
            name: None,
            preview: "same".to_string(),
            cwd: "/repo".to_string(),
            created_at: 15,
            updated_at: 20,
        },
    ];

    let sorted = sort_threads_for_picker(&threads);
    let ids: Vec<&str> = sorted.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, vec!["newer", "same-update-later-created", "older"]);
}
