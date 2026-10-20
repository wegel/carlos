//! Regression tests for Codex transcript reload and reconnect behavior.

use std::io::Cursor;

use serde_json::{json, Value};

use super::{
    handle_notification_line, load_history_from_start_or_resume, parse_rollout_history,
    prefer_complete_rollout_history, AppState, Role,
};

const STALE_REPLAY: &str = include_str!("fixtures/stale_transcript_replay.jsonl");

#[test]
fn later_completed_turn_survives_unfinished_turn_and_duplicate_ordinal() {
    let rollout = parse_rollout_history(Cursor::new(STALE_REPLAY), "thread-root")
        .expect("parse rollout fixture");
    let server = truncated_server_thread(&rollout);
    let chosen = prefer_complete_rollout_history(server, rollout);
    let response = json!({"result": {"thread": chosen}}).to_string();
    let mut app = AppState::new("thread-root".to_string());

    load_history_from_start_or_resume(&mut app, &response).expect("load first snapshot");
    load_history_from_start_or_resume(&mut app, &response).expect("reload same snapshot");

    assert_eq!(message_count(&app, "Committed EP118:"), 1);
    assert_eq!(message_count(&app, "old command output"), 1);
    assert_eq!(message_count(&app, "Preparing vendor knowledge base"), 1);
    assert_eq!(
        message_count(&app, "Updating exclusions and preparing commit"),
        1
    );
    let latest_assistant = app
        .messages
        .iter()
        .rfind(|message| message.role == Role::Assistant)
        .expect("latest assistant message");
    assert!(latest_assistant.text.starts_with("Committed EP118:"));
    assert!(app
        .messages
        .iter()
        .any(|message| message.role == Role::Commentary));
}

#[test]
fn raw_records_cannot_duplicate_lifecycle_items() {
    let rollout = parse_rollout_history(Cursor::new(STALE_REPLAY), "thread-root")
        .expect("parse rollout fixture");
    let response = json!({"result": {"thread": rollout}}).to_string();
    let mut app = AppState::new("thread-root".to_string());
    load_history_from_start_or_resume(&mut app, &response).expect("load snapshot");
    let initial_count = app.messages.len();

    handle_notification_line(
        &mut app,
        r#"{"method":"codex/event/raw_response_item","params":{"msg":{"item":{"type":"function_call_output","call_id":"command-old","output":"old command output"}}}}"#,
    );

    assert_eq!(app.messages.len(), initial_count);
    assert_eq!(message_count(&app, "old command output"), 1);
}

#[test]
fn rollout_without_a_completed_final_answer_cannot_replace_server_history() {
    let server = json!({"id": "thread-root", "turns": [{
        "id": "turn-server",
        "status": "completed",
        "items": [{
            "type": "agentMessage",
            "id": "server-final",
            "phase": "final_answer",
            "text": "server answer"
        }]
    }]});
    let rollout = json!({"id": "thread-root", "turns": [{
        "id": "turn-empty",
        "status": "completed",
        "items": []
    }]});

    let chosen = prefer_complete_rollout_history(server.clone(), rollout);

    assert_eq!(chosen, server);
}

#[test]
fn reconnect_during_stream_replaces_snapshot_and_completed_item_once() {
    let snapshot = json!({
        "result": {"thread": {"id": "thread-root", "turns": [{
            "id": "turn-stream",
            "status": "inProgress",
            "items": [{
                "type": "agentMessage",
                "id": "message-stream",
                "phase": "commentary",
                "text": "partial"
            }]
        }]}}
    })
    .to_string();
    let mut app = AppState::new("thread-root".to_string());
    load_history_from_start_or_resume(&mut app, &snapshot).expect("load streamed snapshot");
    load_history_from_start_or_resume(&mut app, &snapshot).expect("reload streamed snapshot");

    handle_notification_line(
        &mut app,
        r#"{"method":"item/agentMessage/delta","params":{"threadId":"thread-root","turnId":"turn-stream","itemId":"message-stream","delta":" continued"}}"#,
    );
    let completed = r#"{"method":"item/completed","params":{"threadId":"thread-root","turnId":"turn-stream","item":{"type":"agentMessage","id":"message-stream","phase":"final_answer","text":"partial continued"}}}"#;
    handle_notification_line(&mut app, completed);
    handle_notification_line(&mut app, completed);
    handle_notification_line(
        &mut app,
        r#"{"method":"item/agentMessage/delta","params":{"threadId":"thread-root","turnId":"turn-stream","itemId":"message-stream","delta":" continued"}}"#,
    );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::Assistant);
    assert_eq!(app.messages[0].text, "partial continued");
}

#[test]
fn late_turn_completion_cannot_finish_a_newer_active_turn() {
    let mut app = AppState::new("thread-root".to_string());
    handle_notification_line(
        &mut app,
        r#"{"method":"turn/started","params":{"threadId":"thread-root","turn":{"id":"turn-new","status":"inProgress"}}}"#,
    );
    handle_notification_line(
        &mut app,
        r#"{"method":"turn/completed","params":{"threadId":"thread-root","turn":{"id":"turn-old","status":"completed"}}}"#,
    );
    assert_eq!(app.active_turn_id.as_deref(), Some("turn-new"));

    handle_notification_line(
        &mut app,
        r#"{"method":"turn/completed","params":{"threadId":"thread-root","turn":{"id":"turn-new","status":"completed"}}}"#,
    );
    assert_eq!(app.active_turn_id, None);
}

#[test]
fn command_output_completion_replaces_streamed_output() {
    let mut app = AppState::new("thread-root".to_string());
    handle_notification_line(
        &mut app,
        r#"{"method":"item/started","params":{"threadId":"thread-root","turnId":"turn-stream","item":{"type":"commandExecution","id":"command-stream"}}}"#,
    );
    handle_notification_line(
        &mut app,
        r#"{"method":"item/commandExecution/outputDelta","params":{"threadId":"thread-root","turnId":"turn-stream","itemId":"command-stream","delta":"partial"}}"#,
    );
    handle_notification_line(
        &mut app,
        r#"{"method":"item/completed","params":{"threadId":"thread-root","turnId":"turn-stream","item":{"type":"commandExecution","id":"command-stream","aggregatedOutput":"final output","exitCode":1}}}"#,
    );
    handle_notification_line(
        &mut app,
        r#"{"method":"item/commandExecution/outputDelta","params":{"threadId":"thread-root","turnId":"turn-stream","itemId":"command-stream","delta":"partial"}}"#,
    );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::ToolOutput);
    assert_eq!(app.messages[0].text, "final output\nexit code: 1");
}

#[test]
fn notification_for_another_thread_cannot_change_transcript() {
    let mut app = AppState::new("thread-root".to_string());

    handle_notification_line(
        &mut app,
        r#"{"method":"item/completed","params":{"threadId":"thread-other","turnId":"turn-other","item":{"type":"agentMessage","id":"message-other","phase":"final_answer","text":"wrong thread"}}}"#,
    );

    assert!(app.messages.is_empty());
}

#[test]
fn history_removal_markers_cannot_resurrect_edited_turns() {
    let removed = format!(
        "{STALE_REPLAY}\n{}\n",
        json!({"type": "event_msg", "payload": {
            "type": "thread_rolled_back", "num_turns": 1
        }})
    );

    assert!(parse_rollout_history(Cursor::new(removed), "thread-root").is_err());
}

#[test]
fn user_items_keep_their_turn_ids_after_reload_and_live_completion() {
    let rollout = parse_rollout_history(Cursor::new(STALE_REPLAY), "thread-root")
        .expect("parse rollout fixture");
    let response = json!({"result": {"thread": rollout}}).to_string();
    let mut app = AppState::new("thread-root".to_string());
    load_history_from_start_or_resume(&mut app, &response).expect("load snapshot");
    app.enter_rewind_mode();
    assert_eq!(app.rewind_selected_turn_id(), Some("turn-later"));
    app.exit_rewind_mode_restore();

    let completed = r#"{"method":"item/completed","params":{"threadId":"thread-root","turnId":"turn-live","item":{"type":"userMessage","id":"user-live","content":[{"type":"text","text":"Commit EP118"}]}}}"#;
    handle_notification_line(&mut app, completed);
    handle_notification_line(&mut app, completed);
    app.enter_rewind_mode();

    assert_eq!(app.submitted_turn_count(), 2);
    assert_eq!(app.rewind_selected_turn_id(), Some("turn-live"));
}

fn truncated_server_thread(rollout: &Value) -> Value {
    json!({
        "id": "thread-root",
        "turns": [rollout["turns"][0].clone()]
    })
}

fn message_count(app: &AppState, needle: &str) -> usize {
    app.messages
        .iter()
        .filter(|message| message.text.contains(needle))
        .count()
}
