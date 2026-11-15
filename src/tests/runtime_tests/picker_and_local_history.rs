// Tests for the thread picker and local-history-driven session selection.

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
