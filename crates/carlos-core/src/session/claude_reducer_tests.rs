//! Reduce stale helpers, fatal facts and observed model choices without an embedded terminal.

use super::backend_test_support::{action, context};
use super::models::TurnStatus;
use super::state::SessionState;
use crate::claude_backend::{translate_claude_line, ClaudeTranslationState};

fn feed(state: &mut SessionState, line: &str) {
    let update = state.handle_backend_line(line, &context(state));
    assert!(
        action(update).is_none(),
        "these normalized facts need no native follow-up"
    );
}

fn collect_synthetic_lines(lines: &[&str]) -> Vec<String> {
    let mut translator = ClaudeTranslationState::default();
    lines
        .iter()
        .flat_map(|line| {
            translate_claude_line(&mut translator, line)
                .expect("native init")
                .lines
        })
        .collect()
}

#[test]
fn normalized_common_events_from_a_retired_helper_cannot_touch_the_new_branch() {
    let mut app = SessionState::new("old".into(), Default::default(), false);
    feed(&mut app, &serde_json::json!({"method":"claude/sessionChanged","params":{"generation":2,"sessionId":"new"}}).to_string());
    for record in [
        serde_json::json!({"method":"item/completed","params":{"generation":1,"item":{"id":"late","type":"agentMessage","text":"old text"}}}),
        serde_json::json!({"method":"turn/completed","params":{"generation":1,"turn":{"id":"old","status":"completed"}}}),
        serde_json::json!({"method":"thread/tokenUsage/updated","params":{"generation":1,"tokenUsage":{"modelContextWindow":100,"total":{"totalTokens":99}}}}),
    ] {
        feed(&mut app, &record.to_string());
    }
    assert_eq!(app.thread_id(), "new");
    assert!(app.messages().is_empty());
    assert!(app.turn_status_by_id().is_empty());
    assert!(app.context_usage().is_none());
}
#[test]
fn a_fatal_fact_settles_accepted_prompts_before_queued_success_can_arrive() {
    let mut app = SessionState::new("session".into(), Default::default(), false);
    for id in ["active", "queued"] {
        feed(&mut app,&serde_json::json!({"method":"claude/promptSubmitted","params":{"generation":1,"uuid":id,"text":id,"origin":"human"}}).to_string());
    }
    app.record_turn_started("active");
    feed(&mut app,&serde_json::json!({"method":"claude/error","params":{"generation":1,"error":{"message":"fixture fatal error"}}}).to_string());
    for record in [
        serde_json::json!({"method":"item/completed","params":{"generation":1,"item":{"id":"late","type":"agentMessage","text":"late success"}}}),
        serde_json::json!({"method":"turn/completed","params":{"generation":1,"turn":{"id":"active","status":"completed"}}}),
    ] {
        feed(&mut app, &record.to_string());
    }
    assert!(app.active_turn_id().is_none());
    assert!(app.claude_prompts().next().is_none());
    for id in ["active", "queued"] {
        assert_eq!(app.turn_status_by_id().get(id), Some(&TurnStatus::Failed));
    }
    assert!(!app
        .messages()
        .iter()
        .any(|message| message.text == "late success"));
}

#[test]
fn translated_claude_init_preserves_selected_runtime_settings() {
    let synthetic = collect_synthetic_lines(&[
        r#"{"type":"system","subtype":"init","session_id":"session-1","model":"claude-opus-4-7"}"#,
    ]);
    let mut app = SessionState::new("claude-pending-session".into(), Default::default(), false);
    app.set_runtime_settings(
        Some("claude-opus-4-6".to_string()),
        Some("high".to_string()),
        None,
    );

    for line in synthetic {
        feed(&mut app, &line);
    }

    assert_eq!(app.thread_id(), "session-1");
    assert_eq!(app.runtime_settings_label(), "claude-opus-4-6/high");
}
