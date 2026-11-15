// Tests for Claude protocol translation and turn submission.

#[test]
fn submit_turn_text_sends_claude_steer_when_turn_active() {
    let client = ClaudeSteerMock::new();
    let mut app = AppState::new("thread-1".to_string());
    app.session.set_active_turn_id(Some("turn-1".to_string()));

    submit_turn_text(&client, &mut app, "follow up".to_string());

    assert_eq!(
        client.calls.lock().expect("calls lock").as_slice(),
        ["turn/steer"]
    );
    assert_eq!(app.session.messages().len(), 1);
    assert_eq!(app.session.messages()[0].role, Role::User);
    assert_eq!(app.session.messages()[0].text, "follow up");
    assert_eq!(app.input_history_len(), 1);
    assert_eq!(*app.session.status(), "sent steer");
}

#[test]
fn submit_turn_text_queues_behind_existing_claude_turns() {
    let client = ClaudeSteerMock::new();
    let mut app = AppState::new("thread-1".to_string());
    app.queue_turn_input("first");

    submit_turn_text(&client, &mut app, "second".to_string());

    let first = app
        .dequeue_turn_input(Instant::now())
        .expect("first queued turn");
    let second = app
        .dequeue_turn_input(Instant::now())
        .expect("second queued turn");
    assert_eq!(first.text, "first");
    assert_eq!(second.text, "second");
    assert_eq!(*app.session.status(), "queued behind pending Claude turn");
}

#[test]
fn submit_turn_text_does_not_queue_behind_delayed_ralph_continuation() {
    let client = ClaudeStartMock;
    let mut app = AppState::new("thread-1".to_string());
    app.queue_ralph_continuation("continue");

    submit_turn_text(&client, &mut app, "user prompt".to_string());

    assert_eq!(app.session.messages().len(), 1);
    assert_eq!(app.session.messages()[0].text, "user prompt");
    assert_eq!(*app.session.status(), "sent turn");
}

#[test]
fn submit_turn_text_appends_user_message_for_claude_start() {
    let client = ClaudeStartMock;
    let mut app = AppState::new("thread-1".to_string());

    submit_turn_text(&client, &mut app, "hello".to_string());

    assert_eq!(app.session.messages().len(), 1);
    assert_eq!(app.session.messages()[0].role, Role::User);
    assert_eq!(app.session.messages()[0].text, "hello");
}

#[test]
fn submit_turn_text_marks_claude_turn_active_immediately() {
    let client = ClaudeStartMock;
    let mut app = AppState::new("thread-1".to_string());

    submit_turn_text(&client, &mut app, "hello".to_string());

    assert_eq!(
        app.session.active_turn_id().as_deref(),
        Some("claude-turn-pending")
    );
    assert_eq!(app.session.turn_start(), Some(1));
}

#[test]
fn submit_turn_text_backfills_existing_history_for_claude_start() {
    let client = ClaudeStartMock;
    let mut app = AppState::new("thread-1".to_string());
    app.push_input_history("hello");

    submit_turn_text(&client, &mut app, "hello".to_string());

    assert_eq!(app.input_history_len(), 1);
    assert_eq!(app.input_history_message_indices(), &[Some(0)]);
}

#[test]
fn submit_turn_text_skips_history_for_internal_claude_turns() {
    let client = ClaudeStartMock;
    let mut app = AppState::new("thread-1".to_string());

    submit_turn_text_with_history(&client, &mut app, "auto".to_string(), false);

    assert_eq!(app.session.messages().len(), 1);
    assert_eq!(app.input_history_len(), 0);
}

#[test]
fn translated_claude_init_preserves_selected_runtime_settings() {
    let synthetic = collect_synthetic_lines(&[
        r#"{"type":"system","subtype":"init","session_id":"session-1","model":"claude-opus-4-7"}"#,
    ]);
    let mut app = AppState::new("claude-pending-session".to_string());
    app.set_runtime_settings(
        Some("claude-opus-4-6".to_string()),
        Some("high".to_string()),
        None,
    );

    for line in synthetic {
        handle_notification_line(&mut app, &line);
    }

    assert_eq!(app.session.thread_id(), "session-1");
    assert_eq!(app.runtime_settings_label(), "claude-opus-4-6/high");
}
