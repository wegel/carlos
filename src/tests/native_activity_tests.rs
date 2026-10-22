//! Native Codex waits and child-agent records remain visible during live use and replay.

use serde_json::{json, Value};

use super::{handle_notification_line, load_history_from_start_or_resume, AppState, Role};

fn item(app: &mut AppState, method: &str, value: &Value) {
    handle_notification_line(
        app,
        &json!({"method":method, "params": {
        "threadId":"root", "turnId":"turn", "item":value}})
        .to_string(),
    );
}

#[test]
fn child_completion_after_parent_turn_is_visible_and_replay_is_idempotent() {
    let mut app = AppState::new("root".into());
    let started = json!({"type":"subAgentActivity", "id":"spawn", "kind":"started",
        "agentPath":"/root/monitor", "agentThreadId":"child"});
    let mut completed = started.clone();
    completed["id"] = json!("child-finished");
    completed["kind"] = json!("completed");
    handle_notification_line(
        &mut app,
        &json!({"method":"turn/started", "params": {
        "threadId":"root", "turn":{"id":"turn"}}})
        .to_string(),
    );
    item(&mut app, "item/started", &started);
    item(&mut app, "item/completed", &started);
    handle_notification_line(
        &mut app,
        &json!({"method":"turn/completed", "params": {
        "threadId":"root", "turn":{"id":"turn", "status":"completed"}}})
        .to_string(),
    );
    item(&mut app, "item/completed", &completed);
    item(&mut app, "item/completed", &completed);
    assert!(
        app.active_turn_id.is_none(),
        "the client must not invent another agent turn"
    );
    assert_eq!(app.messages.len(), 2);
    assert_eq!(app.messages[1].text, "Agent /root/monitor completed");
    let response = json!({"result":{"thread":{"id":"root", "turns":[{
        "id":"turn", "status":"completed", "items":[started, completed]}]}}})
    .to_string();
    for _ in 0..2 {
        load_history_from_start_or_resume(&mut app, &response).expect("reload native activity");
        assert_eq!(app.messages.len(), 2);
        assert_eq!(app.messages[1].text, "Agent /root/monitor completed");
    }
}

#[test]
fn sleep_and_legacy_agent_calls_merge_completed_items() {
    let mut app = AppState::new("root".into());
    let sleep = json!({"type":"sleep", "id":"sleep", "durationMs":30000});
    item(&mut app, "item/started", &sleep);
    assert_eq!(app.messages[0].text, "Wait up to 30 s");
    item(&mut app, "item/completed", &sleep);
    let mut call = json!({"type":"collabAgentToolCall", "id":"wait", "tool":"wait",
        "status":"inProgress", "agentsStates":{}, "receiverThreadIds":["child"],
        "senderThreadId":"root"});
    item(&mut app, "item/started", &call);
    assert_eq!(app.messages[1].text, "Wait for agent: running");
    call["status"] = json!("completed");
    call["agentsStates"] = json!({"child":{"status":"completed", "message":"Cleanup finished"}});
    item(&mut app, "item/completed", &call);
    item(&mut app, "item/started", &call);
    assert_eq!(app.messages.len(), 2);
    assert!(app.messages[1].text.contains("Cleanup finished"));
    assert_eq!(app.messages[1].role, Role::ToolCall);
}

#[test]
fn child_thread_events_do_not_replace_parent_answers() {
    let mut app = AppState::new("root".into());
    handle_notification_line(
        &mut app,
        &json!({"method":"item/completed", "params": {
        "threadId":"child", "turnId":"child-turn", "item": {
            "type":"agentMessage", "id":"child-answer", "phase":"final_answer",
            "text":"Private child answer"}}})
        .to_string(),
    );
    assert!(app.messages.is_empty());
}
