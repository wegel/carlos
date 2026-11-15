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
