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
fn parse_cli_args_supports_dictation_profile() {
    let parsed = parse_cli_args(vec!["--dictation-profile".to_string(), "fr-qc".to_string()])
        .expect("parse");

    assert_eq!(parsed.dictation_profile.as_deref(), Some("fr-qc"));
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
fn runtime_settings_label_omits_missing_trailing_fields() {
    let mut app = AppState::new("thread-1".to_string());
    app.set_runtime_settings(Some("claude-opus-4-6".to_string()), None, None);

    assert_eq!(app.runtime_settings_label(), "claude-opus-4-6");
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
