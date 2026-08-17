// Local session listing and queued-turn tests for the Grok backend.

#[test]
fn parse_iso_timestamp_secs_reads_rfc3339_prefix() {
    assert_eq!(
        parse_iso_timestamp_secs("2026-08-17T13:00:47.326462776+00:00"),
        1_786_971_647
    );
}

#[test]
fn load_grok_local_history_filters_by_cwd() {
    let root = temp_test_dir("grok-sessions");
    let cwd = Path::new("/repo");
    let other = Path::new("/other");
    let group = root.join("%2Frepo");
    let keep = group.join("session-keep");
    let skip = group.join("session-skip");
    fs::create_dir_all(&keep).expect("keep dir");
    fs::create_dir_all(&skip).expect("skip dir");
    fs::write(
        keep.join("summary.json"),
        r#"{
            "info": { "id": "session-keep", "cwd": "/repo" },
            "session_summary": "Keep me",
            "created_at": "2026-08-17T12:00:00Z",
            "updated_at": "2026-08-17T13:00:00Z"
        }"#,
    )
    .expect("write keep");
    fs::write(
        skip.join("summary.json"),
        r#"{
            "info": { "id": "session-skip", "cwd": "/other" },
            "session_summary": "Skip me",
            "created_at": "2026-08-17T12:00:00Z",
            "updated_at": "2026-08-17T14:00:00Z"
        }"#,
    )
    .expect("write skip");

    let listed = load_grok_thread_summaries_from(root.clone(), cwd).expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].session_id, "session-keep");
    assert_eq!(listed[0].title.as_deref(), Some("Keep me"));

    let other_listed = load_grok_thread_summaries_from(root.clone(), other).expect("list other");
    assert_eq!(other_listed.len(), 1);
    assert_eq!(other_listed[0].session_id, "session-skip");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn submit_turn_text_sends_grok_follow_up_while_turn_active() {
    let client = GrokStartMock::new();
    let mut app = AppState::new("thread-1".to_string());
    app.active_turn_id = Some("turn-1".to_string());

    submit_turn_text(&client, &mut app, "follow up".to_string());

    assert_eq!(
        client.calls.lock().expect("calls lock").as_slice(),
        ["turn/start"]
    );
    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].text, "follow up");
    assert_eq!(app.active_turn_id.as_deref(), Some("turn-1"));
    assert_eq!(app.status, "sent turn");
}

#[test]
fn submit_turn_text_echoes_grok_prompt_and_marks_pending_turn() {
    let client = GrokStartMock::new();
    let mut app = AppState::new("thread-1".to_string());

    submit_turn_text(&client, &mut app, "hello grok".to_string());

    assert_eq!(
        client.calls.lock().expect("calls lock").as_slice(),
        ["turn/start"]
    );
    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::User);
    assert_eq!(app.messages[0].text, "hello grok");
    assert_eq!(app.active_turn_id.as_deref(), Some("grok-turn-pending"));
}

#[test]
fn submit_turn_text_queues_behind_existing_grok_turns() {
    let client = GrokStartMock::new();
    let mut app = AppState::new("thread-1".to_string());
    app.queue_turn_input("first");

    submit_turn_text(&client, &mut app, "second".to_string());

    assert!(client.calls.lock().expect("calls lock").is_empty());
    assert_eq!(app.status, "queued behind pending Grok turn");
}

#[test]
fn parse_rewind_points_reads_snake_case_list() {
    let points = parse_rewind_points(&json!({
        "rewind_points": [
            {
                "prompt_index": 0,
                "prompt_preview": "first"
            },
            {
                "prompt_index": 1,
                "prompt_preview": "second"
            }
        ]
    }));
    assert_eq!(
        points,
        [
            GrokRewindPoint {
                prompt_index: 0,
                prompt_preview: "first".to_string()
            },
            GrokRewindPoint {
                prompt_index: 1,
                prompt_preview: "second".to_string()
            }
        ]
    );
}

#[test]
fn resolve_rewind_target_uses_user_facing_index() {
    let points = vec![
        GrokRewindPoint {
            prompt_index: 0,
            prompt_preview: "<system-reminder>hidden".to_string(),
        },
        GrokRewindPoint {
            prompt_index: 1,
            prompt_preview: "first".to_string(),
        },
        GrokRewindPoint {
            prompt_index: 2,
            prompt_preview: "second".to_string(),
        },
    ];
    assert!(is_hidden_rewind_point("<system-reminder>hidden"));
    assert_eq!(resolve_rewind_target(&points, 1).expect("target"), 2);
}

#[test]
fn submit_rewind_turn_text_keeps_prefix_and_resubmits() {
    let client = GrokRewindMock::new();
    let mut app = AppState::new("thread-original".to_string());
    let u1 = app.append_message(Role::User, "first");
    app.record_input_history("first", Some(u1));
    let _ = app.append_message(Role::Assistant, "reply one");
    let u2 = app.append_message(Role::User, "second");
    app.record_input_history("second", Some(u2));
    let _ = app.append_message(Role::Assistant, "reply two");
    app.set_input_text("edited second");
    app.enter_rewind_mode();
    let rewind_history_idx = app
        .rewind_selected_history_index()
        .expect("rewind history index");

    super::turn_submit::submit_rewind_turn_text(
        &client,
        &mut app,
        rewind_history_idx,
        "edited second".to_string(),
    );

    assert_eq!(app.thread_id, "grok-fork");
    assert_eq!(app.messages.len(), 3);
    assert_eq!(app.messages[0].text, "first");
    assert_eq!(app.messages[1].text, "reply one");
    assert_eq!(app.messages[2].text, "edited second");
    assert_eq!(
        client.fork_calls.lock().expect("fork lock").as_slice(),
        &[RewindForkRequest {
            keep_turns: 1,
            drop_turns: 1,
        }]
    );
    assert_eq!(
        client
            .turn_start_calls
            .lock()
            .expect("start lock")
            .as_slice(),
        ["edited second"]
    );
}

#[test]
fn submit_dequeued_grok_turn_starts_even_when_more_are_queued() {
    let client = GrokStartMock::new();
    let mut app = AppState::new("thread-1".to_string());
    app.queue_turn_input("second");

    submit_dequeued_turn_text(&client, &mut app, "first".to_string(), false);

    assert_eq!(
        client.calls.lock().expect("calls lock").as_slice(),
        ["turn/start"]
    );
    assert_eq!(app.messages[0].text, "first");
    assert_eq!(app.active_turn_id.as_deref(), Some("grok-turn-pending"));
    assert_eq!(
        app.dequeue_turn_input(std::time::Instant::now())
            .expect("remaining")
            .text,
        "second"
    );
}

#[test]
fn submit_turn_text_with_history_false_skips_grok_history() {
    let client = GrokStartMock::new();
    let mut app = AppState::new("thread-1".to_string());

    submit_turn_text_with_history(&client, &mut app, "internal".to_string(), false);

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.input_history_len(), 0);
}
