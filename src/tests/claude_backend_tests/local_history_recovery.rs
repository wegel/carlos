// Tests native tool rows retained at the UI boundary.

#[test]
fn normalized_common_events_from_a_retired_helper_cannot_touch_the_new_branch() {
    let mut app = AppState::new("old".into());
    handle_notification_line(&mut app, &serde_json::json!({"method":"claude/sessionChanged","params":{"generation":2,"sessionId":"new"}}).to_string());
    for record in [
        serde_json::json!({"method":"item/completed","params":{"generation":1,"item":{"id":"late","type":"agentMessage","text":"old text"}}}),
        serde_json::json!({"method":"turn/completed","params":{"generation":1,"turn":{"id":"old","status":"completed"}}}),
        serde_json::json!({"method":"thread/tokenUsage/updated","params":{"generation":1,"tokenUsage":{"modelContextWindow":100,"total":{"totalTokens":99}}}}),
    ] {
        handle_notification_line(&mut app, &record.to_string());
    }
    assert_eq!(app.session.thread_id(), "new");
    assert!(app.session.messages().is_empty());
    assert!(app.session.turn_status_by_id().is_empty());
    assert!(app.session.context_usage().is_none());
}

#[test]
fn a_fatal_fact_settles_accepted_prompts_before_queued_success_can_arrive() {
    let mut app = AppState::new("session".into());
    for id in ["active", "queued"] {
        handle_notification_line(&mut app,&serde_json::json!({"method":"claude/promptSubmitted","params":{"generation":1,"uuid":id,"text":id,"origin":"human"}}).to_string());
    }
    app.record_turn_started("active");
    handle_notification_line(&mut app,&serde_json::json!({"method":"claude/error","params":{"generation":1,"error":{"message":"fixture fatal error"}}}).to_string());
    for record in [
        serde_json::json!({"method":"item/completed","params":{"generation":1,"item":{"id":"late","type":"agentMessage","text":"late success"}}}),
        serde_json::json!({"method":"turn/completed","params":{"generation":1,"turn":{"id":"active","status":"completed"}}}),
    ] {
        handle_notification_line(&mut app, &record.to_string());
    }
    assert!(app.session.active_turn_id().is_none());
    assert!(app.session.claude_prompts().next().is_none());
    for id in ["active", "queued"] {
        assert_eq!(
            app.session.turn_status_by_id().get(id),
            Some(&carlos_core::session::models::TurnStatus::Failed)
        );
    }
    assert!(!app
        .session
        .messages()
        .iter()
        .any(|message| message.text == "late success"));
}
