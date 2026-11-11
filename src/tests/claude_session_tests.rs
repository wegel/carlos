//! Verify native session identity, history sources, and recoverable startup failures through shared routing.

use serde_json::{json, Value};

use super::claude_state::ClaudePhase;
use super::notifications::handle_server_message_line;
use super::state::AppState;

fn send(app: &mut AppState, method: &str, generation: u64, mut params: Value) {
    params["generation"] = json!(generation);
    let _ = handle_server_message_line(app, &json!({"method":method,"params":params}).to_string());
}

fn frame(app: &mut AppState, generation: u64, frame: Value, method: Option<&str>) {
    send(
        app,
        "claude/controller",
        generation,
        json!({"frame":frame,"requestMethod":method}),
    );
}

fn switch(app: &mut AppState) {
    send(
        app,
        "claude/sessionChanged",
        2,
        json!({"sessionId":"new-session"}),
    );
}

#[test]
fn old_generation_close_cancel_and_metadata_cannot_touch_the_new_session() {
    let mut app = AppState::new("original".to_string());
    switch(&mut app);
    frame(
        &mut app,
        2,
        json!({"type":"callback","callbackId":"same-native-id","kind":"permission",
        "payload":{"toolName":"Bash","input":{"command":"new command"}}}),
        None,
    );
    for event in [
        json!({"type":"event","event":"closed","reason":"old-shutdown"}),
        json!({"type":"event","event":"callback_cancelled","callbackId":"same-native-id"}),
        json!({"type":"reply","ok":true,"result":{"models":[{"value":"old-model"}]}}),
    ] {
        frame(&mut app, 1, event, Some("start"));
    }
    assert_eq!(app.thread_id, "new-session");
    assert!(app.claude.phase == ClaudePhase::Starting);
    assert!(app.claude.capabilities.is_none());
    assert_eq!(
        app.approval
            .pending
            .as_ref()
            .expect("new callback remains")
            .request_id,
        "same-native-id"
    );
    assert!(app.claude.answers.drafts.contains_key("same-native-id"));
}

#[test]
fn closed_query_ignores_late_success_and_only_new_generation_can_reopen() {
    let mut app = AppState::new("original".to_string());
    app.claude.phase = ClaudePhase::Starting;
    app.claude.history.edited_prompt = Some("saved edit".to_string());
    frame(
        &mut app,
        1,
        json!({"type":"event","event":"closed","reason":"sdk_eof"}),
        None,
    );
    for method in ["start", "capabilities"] {
        frame(
            &mut app,
            1,
            json!({"type":"reply","ok":true,"result":{"models":[]}}),
            Some(method),
        );
    }
    assert!(app.claude.phase == ClaudePhase::Closed);
    assert_eq!(app.input_text(), "saved edit");
    assert!(app.claude.history.edited_prompt.is_none());
    switch(&mut app);
    assert!(app.claude.phase == ClaudePhase::Starting);
    assert_eq!(app.input_text(), "saved edit");
}

#[test]
fn failures_before_and_after_switch_keep_the_correct_session_and_edit() {
    let mut app = AppState::new("original".to_string());
    app.set_input_text("edited draft");
    app.claude.history.edited_prompt = Some("edited draft".to_string());
    app.claude.phase = ClaudePhase::Starting;
    send(
        &mut app,
        "claude/forkFailed",
        1,
        json!({"message":"source unavailable"}),
    );
    assert_eq!(app.thread_id, "original");
    assert!(app.claude.phase == ClaudePhase::Ready);
    assert_eq!(app.input_text(), "edited draft");
    app.claude.history.edited_prompt = Some("edited draft".to_string());
    switch(&mut app);
    app.clear_input();
    send(
        &mut app,
        "claude/sessionStartFailed",
        2,
        json!({"message":"helper input closed"}),
    );
    assert!(app.claude.phase == ClaudePhase::Closed);
    assert_eq!(app.input_text(), "edited draft");
}

#[test]
fn session_switch_preserves_explicit_choices_and_releases_inherited_observations() {
    for explicit in [false, true] {
        let mut app = AppState::new("original".to_string());
        app.runtime.current_model = Some("sdk-opus".to_string());
        app.runtime.current_effort = Some("high".to_string());
        app.claude.selection.inherits_model = !explicit;
        app.claude.selection.inherits_effort = !explicit;
        switch(&mut app);
        assert_eq!(
            app.runtime.current_model.as_deref(),
            explicit.then_some("sdk-opus")
        );
        assert_eq!(
            app.runtime.current_effort.as_deref(),
            explicit.then_some("high")
        );
        assert_eq!(app.claude.selection.inherits_model, !explicit);
    }
}

#[test]
fn loaded_records_keep_sources_uuid_anchors_and_unknown_metadata_separate() {
    let mut app = AppState::new("session".to_string());
    let rows: Vec<Value> = ["human", "automatic", "unknown", "system"].iter().enumerate().map(|(index, source)| {
        json!({"source":source,"message":{"type":"user","uuid":format!("uuid-{index}"),"session_id":"session",
            "futureMetadata":{"retained":true}, "message":{"role":"user","content":format!("text {index}")}}})
    }).collect();
    frame(
        &mut app,
        1,
        json!({"type":"reply","ok":true,"result":{"messages":rows,"nextOffset":null}}),
        Some("sessions.read"),
    );
    app.enter_rewind_mode();
    assert_eq!(app.input_text(), "text 0");
    assert_eq!(app.rewind_selected_turn_id(), Some("uuid-0"));
    assert_eq!(app.claude.history.unknown.len(), 1);
    assert_eq!(app.claude.history.unknown[0].uuid, "uuid-2");
    assert_eq!(app.claude.history.records.len(), 4);
    assert_eq!(
        app.claude.history.records[3]["futureMetadata"]["retained"],
        true
    );
}

#[test]
fn history_or_start_reply_failure_restores_the_edit_without_forging_a_turn() {
    for method in ["sessions.info", "sessions.read", "start"] {
        let mut app = AppState::new("session".to_string());
        app.claude.phase = ClaudePhase::Starting;
        app.claude.history.edited_prompt = Some("recover me".to_string());
        frame(
            &mut app,
            1,
            json!({"type":"reply","ok":false,"error":{"message":"Fixture failure"}}),
            Some(method),
        );
        assert!(app.claude.phase == ClaudePhase::Closed);
        assert_eq!(app.input_text(), "recover me");
        assert!(app.active_turn_id.is_none());
    }
}

#[test]
fn closed_startup_recovers_queued_humans_without_replacing_the_new_composer() {
    let mut app = AppState::new("session".to_string());
    app.claude.phase = ClaudePhase::Starting;
    app.queue_turn_input("first human");
    app.queue_turn_input("second human");
    app.queue_ralph_continuation("automatic should disappear");
    app.set_input_text("newer draft");
    frame(
        &mut app,
        1,
        json!({"type":"event","event":"closed","reason":"start_failed"}),
        None,
    );
    assert_eq!(app.input_text(), "newer draft");
    super::claude_session_history::load_unsent(&mut app);
    assert_eq!(app.input_text(), "newer draft");
    for expected in ["first human", "second human"] {
        app.clear_input();
        super::claude_session_history::load_unsent(&mut app);
        assert_eq!(app.input_text(), expected);
    }
    app.clear_input();
    super::claude_session_history::load_unsent(&mut app);
    assert!(app.input_is_empty());
}

#[test]
fn failed_edit_with_newer_composer_can_be_recovered_once_without_overwriting_it() {
    let mut app = AppState::new("original".to_string());
    app.claude.history.edited_prompt = Some("edited before startup".to_string());
    switch(&mut app);
    app.set_input_text("newer composer");
    send(
        &mut app,
        "claude/sessionStartFailed",
        2,
        json!({"message":"startup failed"}),
    );
    frame(
        &mut app,
        2,
        json!({"type":"event","event":"closed","reason":"start_failed"}),
        None,
    );
    assert_eq!(app.input_text(), "newer composer");
    app.clear_input();
    super::claude_session_history::load_unsent(&mut app);
    assert_eq!(app.input_text(), "edited before startup");
    app.clear_input();
    super::claude_session_history::load_unsent(&mut app);
    assert!(app.input_is_empty());
}
