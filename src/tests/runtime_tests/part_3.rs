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
    let continuation = app
        .dequeue_turn_input(deadline)
        .expect("queued continuation");
    assert_eq!(continuation.text, "continue");
}

#[test]
fn ready_ralph_continuation_stays_ahead_of_later_user_turns() {
    let mut app = AppState::new("thread-1".to_string());
    app.queue_ralph_continuation("continue");
    let deadline = app
        .ralph_pending_continuation_deadline()
        .expect("continuation deadline");

    app.promote_ready_continuation(deadline);
    app.queue_turn_input("later");

    let continuation = app
        .dequeue_turn_input(std::time::Instant::now())
        .expect("continuation");
    assert_eq!(continuation.text, "continue");
    let later = app
        .dequeue_turn_input(std::time::Instant::now())
        .expect("later user turn");
    assert_eq!(later.text, "later");
}

#[test]
fn promoted_ralph_continuation_still_counts_as_pending_until_sent() {
    let mut app = AppState::new("thread-1".to_string());
    app.queue_ralph_continuation("continue");
    let deadline = app
        .ralph_pending_continuation_deadline()
        .expect("continuation deadline");

    app.promote_ready_continuation(deadline);
    assert!(app.has_pending_ralph_continuation());

    let continuation = app
        .dequeue_turn_input(std::time::Instant::now())
        .expect("continuation");
    assert_eq!(continuation.text, "continue");
    assert!(!app.has_pending_ralph_continuation());
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
        resume_hint(Backend::Codex, "xxxx-xxxx"),
        "to resume this session use:\ncarlos --backend codex resume xxxx-xxxx"
    );
}

#[test]
fn styled_resume_hint_colors_only_command_line() {
    assert_eq!(
        styled_resume_hint(Backend::Claude, "xxxx-xxxx"),
        "to resume this session use:\n\x1b[94mcarlos --backend claude resume xxxx-xxxx\x1b[0m"
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

#[test]
fn load_claude_thread_summaries_reads_local_session_metadata() {
    let _guard = runtime_env_lock().lock().expect("env lock");
    let old_home = std::env::var_os("HOME");
    let temp_root = std::env::temp_dir().join(format!(
        "carlos-claude-picker-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("duration")
            .as_nanos()
    ));
    let cwd = "/repo";
    let project_dir = temp_root.join(".claude").join("projects").join(
        crate::claude_backend::claude_project_dir_name(std::path::Path::new(cwd)),
    );
    std::fs::create_dir_all(&project_dir).expect("create project dir");
    std::fs::write(
        project_dir.join("session-a.jsonl"),
        concat!(
            "{\"type\":\"user\",\"customTitle\":\"media-pipeline-worker-pool\",\"message\":{\"role\":\"user\",\"content\":\"Investigate streaming slowdown\"},\"cwd\":\"/repo\"}\n",
            "{\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"Looking into it.\"}]},\"cwd\":\"/repo\"}\n"
        ),
    )
    .expect("write session a");
    std::fs::write(
        project_dir.join("session-b.jsonl"),
        "{\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":\"Second session\"},\"cwd\":\"/repo\",\"slug\":\"sleepy-river-otter\"}\n",
    )
    .expect("write session b");
    std::env::set_var("HOME", &temp_root);

    let mut threads =
        load_claude_thread_summaries(std::path::Path::new(cwd), cwd).expect("load summaries");
    threads.sort_by(|lhs, rhs| lhs.id.cmp(&rhs.id));

    assert_eq!(threads.len(), 2);
    assert_eq!(threads[0].id, "session-a");
    assert_eq!(
        threads[0].name.as_deref(),
        Some("media-pipeline-worker-pool")
    );
    assert_eq!(threads[0].preview, "Investigate streaming slowdown");
    assert_eq!(threads[1].id, "session-b");
    assert_eq!(threads[1].name.as_deref(), Some("sleepy-river-otter"));
    assert_eq!(threads[1].preview, "Second session");

    match old_home {
        Some(value) => std::env::set_var("HOME", value),
        None => std::env::remove_var("HOME"),
    }
    let _ = std::fs::remove_dir_all(temp_root);
}
