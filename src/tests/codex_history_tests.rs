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
    let chosen = prefer_complete_rollout_history(server, rollout).expect("eligible legacy merge");
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

    let chosen =
        prefer_complete_rollout_history(server.clone(), rollout).expect("eligible legacy merge");

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

#[test]
fn legacy_recovery_keeps_server_prefix_metadata_and_fills_empty_completed_items() {
    let rollout =
        parse_rollout_history(Cursor::new(STALE_REPLAY), "thread-root").expect("raw fixture");
    let server = json!({"id":"thread-root","name":"keep this name","future":42,
        "turns":[{"id":"inherited-prefix","status":"completed","items":[{"id":"ancestor","type":"agentMessage","text":"inherited"}]},
        {"id":"turn-later","status":"completed","items":[],"future":"keep"}]});
    let recovered =
        prefer_complete_rollout_history(server, rollout).expect("eligible legacy merge");
    assert_eq!(recovered["name"], "keep this name");
    assert_eq!(recovered["future"], 42);
    assert_eq!(recovered["turns"][0]["id"], "inherited-prefix");
    assert_eq!(recovered["turns"][1]["future"], "keep");
    assert!(recovered["turns"][1]["items"]
        .as_array()
        .expect("items")
        .iter()
        .any(|item| item["id"] == "final-later"));
    assert_eq!(recovered["turns"].as_array().expect("turns").len(), 2);
}

#[test]
fn inherited_raw_forks_cannot_replace_server_ancestry() {
    for record in [
        json!({"type":"history_base","payload":{"end_ordinal_exclusive":40}}),
        json!({"type":"session_meta","payload":{"forked_from_id":"source"}}),
    ] {
        let raw = format!("{record}\n{STALE_REPLAY}");
        assert!(parse_rollout_history(Cursor::new(raw), "thread-root").is_err());
    }
}

#[test]
fn legacy_nonprefix_items_keep_raw_order_and_authoritative_server_versions() {
    let server = json!({"id":"root","name":"keep","turns":[{"id":"t","status":"completed","items":[
        {"id":"b","type":"commandExecution","command":"native"},
        {"id":"unknown","type":"future","extra":42}]}]});
    let raw = json!({"id":"root","turns":[{"id":"t","status":"completed","items":[
        {"id":"a","type":"userMessage"},{"id":"b","type":"commandExecution","command":"raw"},
        {"id":"c","type":"agentMessage","phase":"final_answer","text":"done"}]}]});
    let recovered = prefer_complete_rollout_history(server, raw).expect("eligible legacy merge");
    let items = recovered["turns"][0]["items"].as_array().expect("items");
    assert_eq!(
        items
            .iter()
            .map(|item| item["id"].as_str().expect("ID"))
            .collect::<Vec<_>>(),
        ["a", "b", "unknown", "c"]
    );
    assert_eq!(items[1]["command"], "native");
    assert_eq!(items[2]["extra"], 42);
}

#[test]
fn legacy_item_ids_are_scoped_to_their_native_turn() {
    let next = [
        json!({"type":"task_started","turn_id":"other"}),
        json!({"type":"item_completed","turn_id":"other","item":{"id":"final-later","type":"AgentMessage","phase":"final_answer","content":[{"type":"Text","text":"other answer"}]}}),
        json!({"type":"task_complete","turn_id":"other"}),
    ];
    let mut rows = STALE_REPLAY.to_string();
    for payload in next {
        rows.push_str(&format!(
            "{}\n",
            json!({"type":"event_msg","payload":payload})
        ));
    }
    let history = parse_rollout_history(Cursor::new(rows), "thread-root").expect("scoped IDs");
    let turns = history["turns"].as_array().expect("turns");
    assert_eq!(turns.len(), 3);
    assert!(turns[1]["items"]
        .as_array()
        .expect("items")
        .iter()
        .any(|item| item["id"] == "final-later"
            && item["content"][0]["text"]
                .as_str()
                .expect("text")
                .starts_with("Committed EP118")));
    assert_eq!(turns[2]["items"][0]["content"][0]["text"], "other answer");
}

#[test]
fn legacy_recovery_rejects_conflicting_shared_item_order() {
    let a = json!({"id":"a","type":"userMessage"});
    let b = json!({"id":"b","type":"commandExecution"});
    let c = json!({"id":"c","type":"agentMessage","phase":"final_answer"});
    let server = json!({"turns":[{"id":"t","status":"completed","items":[b.clone(),a.clone()]}]});
    let raw = json!({"turns":[{"id":"t","status":"completed","items":[a,b,c]}]});
    assert!(prefer_complete_rollout_history(server, raw)
        .expect_err("contradictory order")
        .to_string()
        .contains("disagree on item order"));
}

#[test]
fn terminal_codex_close_fences_late_turn_items_callbacks_and_keeps_the_composer() {
    let mut app = AppState::new("thread-root".into());
    app.set_input_text("keep my 日本語 draft");
    handle_notification_line(&mut app,&json!({"method":"turn/started","params":{"threadId":"thread-root","turn":{"id":"active"}}}).to_string());
    handle_notification_line(
        &mut app,
        &json!({"method":"carlos/codex/closed","params":{"message":"native EOF"}}).to_string(),
    );
    assert!(app.codex.closed);
    let count = app.messages.len();
    for event in [
        json!({"method":"turn/started","params":{"threadId":"thread-root","turn":{"id":"late"}}}),
        json!({"method":"item/completed","params":{"threadId":"thread-root","turnId":"late","item":{"type":"agentMessage","id":"late-text","text":"must not reactivate"}}}),
        json!({"id":"late-callback","method":"item/commandExecution/requestApproval","params":{"threadId":"thread-root","turnId":"late","itemId":"tool","command":"echo hi"}}),
        json!({"method":"turn/completed","params":{"threadId":"thread-root","turn":{"id":"late","status":"completed"}}}),
    ] {
        handle_notification_line(&mut app, &event.to_string());
    }
    assert!(app.active_turn_id.is_none());
    assert!(app.approval.pending.is_none());
    assert_eq!(app.messages.len(), count);
    assert_eq!(app.input.lines().join("\n"), "keep my 日本語 draft");
}
