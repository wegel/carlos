// CLI handshake and session-history-file replay tests for the Claude backend.

#[test]
fn parse_cli_args_supports_backend_flag() {
    let parsed = parse_cli_args(vec![
        "--backend".to_string(),
        "claude".to_string(),
        "continue".to_string(),
    ])
    .expect("parse");

    assert_eq!(parsed.backend, Backend::Claude);
    assert!(parsed.mode_continue);
}

#[test]
fn parse_cli_args_uses_backend_env_default() {
    let _guard = env_lock().lock().expect("env lock");
    let old = std::env::var_os("CARLOS_BACKEND");
    std::env::set_var("CARLOS_BACKEND", "claude");

    let parsed = parse_cli_args(Vec::<String>::new()).expect("parse");
    assert_eq!(parsed.backend, Backend::Claude);

    match old {
        Some(value) => std::env::set_var("CARLOS_BACKEND", value),
        None => std::env::remove_var("CARLOS_BACKEND"),
    }
}

#[test]
fn build_claude_command_disallows_plan_mode_by_default() {
    let _guard = env_lock().lock().expect("env lock");
    let old = std::env::var_os("CARLOS_CLAUDE_ALLOW_PLAN_MODE");
    std::env::remove_var("CARLOS_CLAUDE_ALLOW_PLAN_MODE");

    let command = build_claude_command_for_test(Path::new("/repo"), &ClaudeLaunchMode::Continue);
    let args = command_args(&command);
    assert!(args.windows(3).any(|window| {
        window[0] == "--disallowedTools"
            && window[1] == "EnterPlanMode"
            && window[2] == "Agent(Plan)"
    }));

    match old {
        Some(value) => std::env::set_var("CARLOS_CLAUDE_ALLOW_PLAN_MODE", value),
        None => std::env::remove_var("CARLOS_CLAUDE_ALLOW_PLAN_MODE"),
    }
}

#[test]
fn build_claude_command_allows_plan_mode_when_opted_in() {
    let _guard = env_lock().lock().expect("env lock");
    let old = std::env::var_os("CARLOS_CLAUDE_ALLOW_PLAN_MODE");
    std::env::set_var("CARLOS_CLAUDE_ALLOW_PLAN_MODE", "1");

    let command = build_claude_command_for_test(Path::new("/repo"), &ClaudeLaunchMode::Continue);
    let args = command_args(&command);
    assert!(!args.iter().any(|arg| arg == "--disallowedTools"));

    match old {
        Some(value) => std::env::set_var("CARLOS_CLAUDE_ALLOW_PLAN_MODE", value),
        None => std::env::remove_var("CARLOS_CLAUDE_ALLOW_PLAN_MODE"),
    }
}

#[test]
fn claude_model_catalog_includes_current_claude_code_models() {
    let catalog = claude_model_catalog();
    let models = catalog
        .iter()
        .map(|model| model.model.as_str())
        .collect::<Vec<_>>();

    assert_eq!(models.first().copied(), Some("opus"));
    assert!(catalog.first().is_some_and(|model| model.is_default));
    for expected in [
        "opus",
        "opusplan",
        "sonnet",
        "sonnet[1m]",
        "haiku",
        "claude-opus-4-7",
        "claude-opus-4-7[1m]",
        "claude-opus-4-6",
        "claude-sonnet-4-6",
        "claude-haiku-4-5",
    ] {
        assert!(models.contains(&expected), "missing {expected}");
    }
}

#[test]
fn probe_claude_startup_reports_early_stderr_exit() {
    let mut command = std::process::Command::new("sh");
    command.args(["-c", "printf 'startup failed\\n' >&2; exit 1"]);

    let err = probe_claude_startup_for_test(&mut command).expect_err("startup should fail");
    assert!(err.to_string().contains("startup failed"));
}

#[test]
fn probe_claude_startup_accepts_first_stdout_line() {
    let mut command = std::process::Command::new("sh");
    command.args(["-c", "printf 'ready\\n'; sleep 1"]);

    probe_claude_startup_for_test(&mut command).expect("startup should succeed");
}

#[test]
fn load_claude_local_history_uses_tilde_claude_config_dir() {
    let _guard = env_lock().lock().expect("env lock");
    let old_home = std::env::var_os("HOME");
    let old_config_dir = std::env::var_os("CLAUDE_CONFIG_DIR");
    let root = temp_test_dir("claude-config-dir-tilde");
    let home = root.join("home");
    let config_dir = home.join(".claude-tt");
    let projects_root = config_dir.join("projects");
    let cwd = Path::new("/repo");
    let session_id = "session-tilde-config";
    let _ = fs::create_dir_all(&projects_root);
    write_session_file(
        &projects_root,
        cwd,
        session_id,
        &[
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Hello from custom config"}]},"sessionId":"session-tilde-config"}"#,
        ],
    );

    std::env::set_var("HOME", &home);
    std::env::set_var("CLAUDE_CONFIG_DIR", "~/.claude-tt");

    let imported =
        load_claude_local_history(cwd, &ClaudeLaunchMode::Resume(session_id.to_string()))
            .expect("history")
            .expect("imported history");
    assert_eq!(imported.session_id, session_id);
    assert_eq!(imported.imported_item_count, 1);

    match old_home {
        Some(value) => std::env::set_var("HOME", value),
        None => std::env::remove_var("HOME"),
    }
    match old_config_dir {
        Some(value) => std::env::set_var("CLAUDE_CONFIG_DIR", value),
        None => std::env::remove_var("CLAUDE_CONFIG_DIR"),
    }
}

#[test]
fn local_claude_history_follows_latest_active_branch() {
    let root = temp_test_dir("claude-history-branch-dag");
    let projects_root = root.join("projects");
    let cwd = Path::new("/repo");
    let session_id = "session-branch-dag";
    write_session_file(
        &projects_root,
        cwd,
        session_id,
        &[
            r#"{"type":"permission-mode","permissionMode":"default","sessionId":"session-branch-dag"}"#,
            r#"{"parentUuid":null,"isSidechain":false,"type":"user","message":{"role":"user","content":"first"},"uuid":"user-1","sessionId":"session-branch-dag"}"#,
            r#"{"parentUuid":"user-1","isSidechain":false,"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"reply one"}]},"uuid":"assistant-1","sessionId":"session-branch-dag"}"#,
            r#"{"parentUuid":"assistant-1","isSidechain":false,"type":"system","subtype":"stop_hook_summary","uuid":"stop-1","sessionId":"session-branch-dag"}"#,
            r#"{"parentUuid":"stop-1","isSidechain":false,"type":"user","message":{"role":"user","content":"another"},"uuid":"user-2-old","sessionId":"session-branch-dag"}"#,
            r#"{"parentUuid":"user-2-old","isSidechain":false,"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"old branch reply"}]},"uuid":"assistant-2-old","sessionId":"session-branch-dag"}"#,
            r#"{"parentUuid":"assistant-2-old","isSidechain":false,"type":"system","subtype":"stop_hook_summary","uuid":"stop-2-old","sessionId":"session-branch-dag"}"#,
            r#"{"parentUuid":"stop-1","isSidechain":false,"type":"user","message":{"role":"user","content":"output a 2 line story"},"uuid":"user-2-new","sessionId":"session-branch-dag"}"#,
            r#"{"parentUuid":"user-2-new","isSidechain":false,"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"new branch reply"}]},"uuid":"assistant-2-new","sessionId":"session-branch-dag"}"#,
            r#"{"parentUuid":"assistant-2-new","isSidechain":false,"type":"system","subtype":"stop_hook_summary","uuid":"stop-2-new","sessionId":"session-branch-dag"}"#,
        ],
    );

    let imported = load_claude_local_history_from_projects_root(
        &projects_root,
        cwd,
        &ClaudeLaunchMode::Resume(session_id.to_string()),
    )
    .expect("import")
    .expect("history");

    let items = imported.thread["turns"][0]["items"]
        .as_array()
        .expect("thread items");
    assert_eq!(items.len(), 4);
    assert_eq!(items[0]["content"][0]["text"], "first");
    assert_eq!(items[1]["text"], "reply one");
    assert_eq!(items[2]["content"][0]["text"], "output a 2 line story");
    assert_eq!(items[3]["text"], "new branch reply");
    let rendered = serde_json::to_string(items).expect("serialize items");
    assert!(!rendered.contains("another"));
    assert!(!rendered.contains("old branch reply"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn local_claude_history_ignores_synthetic_interrupt_records() {
    let root = temp_test_dir("claude-history-interrupted-user");
    let projects_root = root.join("projects");
    let cwd = Path::new("/repo");
    let session_id = "session-interrupted-user";
    write_session_file(
        &projects_root,
        cwd,
        session_id,
        &[
            r#"{"parentUuid":null,"isSidechain":false,"type":"user","message":{"role":"user","content":"first"},"uuid":"user-1","sessionId":"session-interrupted-user"}"#,
            r#"{"parentUuid":"user-1","isSidechain":false,"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"reply one"}]},"uuid":"assistant-1","sessionId":"session-interrupted-user"}"#,
            r#"{"parentUuid":"assistant-1","isSidechain":false,"type":"user","message":{"role":"user","content":[{"type":"text","text":"second"}]},"uuid":"user-2","sessionId":"session-interrupted-user"}"#,
            r#"{"type":"last-prompt","lastPrompt":"second","leafUuid":"user-2","sessionId":"session-interrupted-user"}"#,
            r#"{"parentUuid":"user-2","isSidechain":false,"type":"user","message":{"role":"user","content":[{"type":"text","text":"[Request interrupted by user]"}]},"uuid":"interrupt-1","sessionId":"session-interrupted-user"}"#,
        ],
    );

    let imported = load_claude_local_history_from_projects_root(
        &projects_root,
        cwd,
        &ClaudeLaunchMode::Resume(session_id.to_string()),
    )
    .expect("import")
    .expect("history");

    let items = imported.thread["turns"][0]["items"]
        .as_array()
        .expect("thread items");
    assert_eq!(items.len(), 3);
    assert_eq!(items[0]["content"][0]["text"], "first");
    assert_eq!(items[1]["text"], "reply one");
    assert_eq!(items[2]["content"][0]["text"], "second");
    let rendered = serde_json::to_string(items).expect("serialize items");
    assert!(!rendered.contains("[Request interrupted by user]"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn fork_claude_local_history_branches_from_selected_prefix() {
    let root = temp_test_dir("claude-fork-history");
    let projects_root = root.join("projects");
    let cwd = Path::new("/repo");
    let source_session_id = "session-source";
    let source_path = write_session_file(
        &projects_root,
        cwd,
        source_session_id,
        &[
            r#"{"type":"permission-mode","permissionMode":"default","sessionId":"session-source"}"#,
            r#"{"parentUuid":null,"isSidechain":false,"type":"user","message":{"role":"user","content":"first"},"uuid":"user-1","sessionId":"session-source"}"#,
            r#"{"parentUuid":"user-1","isSidechain":false,"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"reply one"}]},"uuid":"assistant-1","sessionId":"session-source"}"#,
            r#"{"parentUuid":"assistant-1","isSidechain":false,"type":"system","subtype":"stop_hook_summary","uuid":"stop-1","sessionId":"session-source"}"#,
            r#"{"type":"summary","summary":"compact context","leafUuid":"stop-1","sessionId":"session-source"}"#,
            r#"{"parentUuid":"stop-1","isSidechain":false,"type":"user","message":{"role":"user","content":"second-old"},"uuid":"user-2-old","sessionId":"session-source"}"#,
            r#"{"parentUuid":"user-2-old","isSidechain":false,"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"reply old"}]},"uuid":"assistant-2-old","sessionId":"session-source"}"#,
            r#"{"type":"last-prompt","lastPrompt":"second-old","sessionId":"session-source"}"#,
            r#"{"parentUuid":"stop-1","isSidechain":false,"type":"user","message":{"role":"user","content":"second-new"},"uuid":"user-2-new","sessionId":"session-source"}"#,
            r#"{"parentUuid":"user-2-new","isSidechain":false,"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"reply new"}]},"uuid":"assistant-2-new","sessionId":"session-source"}"#,
        ],
    );
    let source_support_dir = source_path.with_extension("");
    fs::create_dir_all(&source_support_dir).expect("create support dir");
    fs::write(
        source_support_dir.join("child.jsonl"),
        "{\"type\":\"note\",\"sessionId\":\"session-source\"}\n",
    )
    .expect("write support jsonl");

    let forked =
        fork_claude_local_history_from_projects_root(&projects_root, cwd, source_session_id, 1)
            .expect("fork history");

    assert_ne!(forked.session_id, source_session_id);
    let items = forked.thread["turns"][0]["items"]
        .as_array()
        .expect("thread items");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["content"][0]["text"], "first");
    assert_eq!(items[1]["text"], "reply one");

    let forked_path = projects_root
        .join(claude_project_dir_name(cwd))
        .join(format!("{}.jsonl", forked.session_id));
    let forked_lines: Vec<Value> = fs::read_to_string(&forked_path)
        .expect("read forked session")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("parse forked line"))
        .collect();
    assert_eq!(forked_lines.len(), 5);
    assert_eq!(
        forked_lines[0]["sessionId"].as_str(),
        Some(forked.session_id.as_str())
    );
    let fork_user_uuid = forked_lines[1]["uuid"].as_str().expect("fork user uuid");
    let fork_assistant_uuid = forked_lines[2]["uuid"]
        .as_str()
        .expect("fork assistant uuid");
    let fork_stop_uuid = forked_lines[3]["uuid"].as_str().expect("fork stop uuid");
    assert_ne!(fork_user_uuid, "user-1");
    assert_ne!(fork_assistant_uuid, "assistant-1");
    assert_ne!(fork_stop_uuid, "stop-1");
    assert_eq!(forked_lines[1]["parentUuid"], Value::Null);
    assert_eq!(forked_lines[2]["parentUuid"].as_str(), Some(fork_user_uuid));
    assert_eq!(
        forked_lines[3]["parentUuid"].as_str(),
        Some(fork_assistant_uuid)
    );
    assert_eq!(forked_lines[4]["leafUuid"].as_str(), Some(fork_stop_uuid));
    let forked_body = serde_json::to_string(&forked_lines).expect("serialize fork");
    assert!(!forked_body.contains("second-old"));
    assert!(!forked_body.contains("second-new"));
    assert!(!forked_body.contains(source_session_id));

    let copied_support = fs::read_to_string(forked_path.with_extension("").join("child.jsonl"))
        .expect("read copied support jsonl");
    assert!(copied_support.contains(&forked.session_id));
    assert!(!copied_support.contains(source_session_id));

    let _ = fs::remove_dir_all(root);
}
