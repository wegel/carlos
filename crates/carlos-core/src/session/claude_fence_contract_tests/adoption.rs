//! Keep native session adoption, reset and diagnostic rules in the Core reducer.

use serde_json::{json, Value};

use super::{accepts, app};
use crate::session::claude::ClaudePhase;
use crate::session::state::SessionState;

#[test]
fn session_changed_requires_newer_generation_and_a_string_identity() {
    for generation in [json!(0), json!(1), json!(2), Value::Null, json!("2")] {
        for id in [
            None,
            Some(Value::Null),
            Some(json!(7)),
            Some(json!("branch")),
            Some(json!("")),
        ] {
            let mut app = SessionState::new("original".into(), Default::default(), false);
            app.append_message(crate::session::models::Role::Assistant, "retained");
            let mut params = json!({"generation":generation});
            if let Some(id) = &id {
                params["sessionId"] = id.clone();
            }
            app.receive_named_claude("claude/sessionChanged", &params, &mut Default::default());
            let adopted = generation == 2 && id.as_ref().is_some_and(Value::is_string);
            assert_eq!(
                app.thread_id(),
                if adopted {
                    id.as_ref().and_then(Value::as_str).expect("string")
                } else {
                    "original"
                }
            );
            assert_eq!(
                app.messages().iter().any(|m| m.text == "retained"),
                !adopted
            );
            assert!(app.claude().enabled());
        }
    }
}

#[test]
fn repeated_reset_advances_epoch_without_retiring_the_transcript_again() {
    let mut app = app();
    for epoch in [5, 6] {
        let params = json!({"generation":1,"epoch":epoch,"new_conversation_id":"same"});
        app.receive_named_claude("claude/conversationReset", &params, &mut Default::default());
        if epoch == 5 {
            app.append_message(crate::session::models::Role::Assistant, "after reset");
        }
    }
    assert_eq!(app.messages().len(), 1);
    assert_eq!(app.messages()[0].text, "after reset");
    assert!(!accepts(
        &mut app,
        "claude/historyRecord",
        &json!({"generation":1,"epoch":5})
    ));
}

#[test]
fn init_adopts_only_while_awaiting_a_reset_and_only_with_a_string_id() {
    for waiting in [false, true] {
        for id in [
            None,
            Some(Value::Null),
            Some(json!(7)),
            Some(json!("native")),
            Some(json!("")),
        ] {
            let mut app = app();
            if waiting {
                app.receive_claude_reset(
                    &json!({"new_conversation_id":"reset"}),
                    &mut Default::default(),
                );
            }
            let mut facts = json!({});
            if let Some(id) = &id {
                facts["session_id"] = id.clone();
            }
            app.receive_claude_initialized(&facts, &mut Default::default());
            let adopted = waiting && id.as_ref().is_some_and(Value::is_string);
            assert_eq!(
                app.thread_id(),
                if adopted {
                    id.as_ref().and_then(Value::as_str).expect("string")
                } else {
                    "original"
                }
            );
            assert_eq!(
                app.claude().phase(),
                if waiting && !adopted {
                    ClaudePhase::Starting
                } else {
                    ClaudePhase::Ready
                }
            );
        }
    }
}

#[test]
fn rejected_reset_cannot_advance_the_epoch_and_missing_epoch_retains_it() {
    let mut app = app();
    for params in [
        json!({"generation":0,"epoch":9}),
        json!({"generation":1,"epoch":3}),
    ] {
        assert!(!accepts(&mut app, "claude/conversationReset", &params));
    }
    assert!(accepts(
        &mut app,
        "claude/conversationReset",
        &json!({"generation":1})
    ));
    assert!(accepts(
        &mut app,
        "claude/historyRecord",
        &json!({"generation":1,"epoch":4})
    ));
    app.close_claude();
    assert!(!accepts(
        &mut app,
        "claude/conversationReset",
        &json!({"generation":1,"epoch":9})
    ));
    app.set_claude_phase_for_test(ClaudePhase::Ready);
    assert!(accepts(
        &mut app,
        "claude/historyRecord",
        &json!({"generation":1,"epoch":4})
    ));
}

#[test]
fn diagnostic_defaults_keep_unsupported_dialogs_open_and_close_after_recovery() {
    for (method, params, phase, row, status) in [
        (
            "claude/error",
            json!({"error":{"code":"unsupported_dialog"}}),
            ClaudePhase::Ready,
            "Unsupported Claude dialog (request unknown, kind unknown)",
            "",
        ),
        (
            "claude/error",
            json!({"error":{}}),
            ClaudePhase::Closed,
            "Claude SDK error",
            "Claude SDK error",
        ),
        (
            "claude/closed",
            json!({}),
            ClaudePhase::Closed,
            "Claude closed: unknown",
            "Claude closed: unknown",
        ),
        (
            "claude/stderr",
            json!({"text":"line\n\u{001b}"}),
            ClaudePhase::Ready,
            "line\n\\u{1b}",
            "",
        ),
    ] {
        let mut app = SessionState::new("original".into(), Default::default(), false);
        app.receive_named_claude(method, &params, &mut Default::default());
        assert_eq!(app.claude().phase(), phase);
        assert_eq!(app.messages()[0].text, row);
        assert_eq!(app.status(), status);
    }
}
