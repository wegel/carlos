//! Enumerate generation, epoch, closed, reset, and session-adoption gates.

use carlos_core::session::claude::ClaudePhase;
use serde_json::{json, Value};

use super::claude_notifications::{handle, route, ClaudeRoute};
use super::state::AppState;

// Every named fact except sessionChanged, which has its own adoption gate.
const FACTS: &[(&str, bool, bool)] = &[
    ("sessionStartFailed", false, true),
    ("forkFailed", false, true),
    ("promptSubmitted", false, true),
    ("promptRejected", false, true),
    ("callback", true, false),
    ("callbackCancelled", true, true),
    ("historyRecord", false, false),
    ("historyFactPart", false, false),
    ("historyPrompt", false, false),
    ("historyCompacted", false, false),
    ("stderr", false, true),
    ("error", false, true),
    ("closed", false, true),
    ("quota", false, false),
    ("browserComplete", true, false),
    ("task", true, false),
    ("conversationReset", false, false),
    ("activity", false, false),
    ("status", false, false),
    ("thinking", false, false),
    ("result", false, false),
    ("usageReport", false, false),
    ("notice", false, false),
    ("initialized", false, false),
    ("promptsConsumed", false, false),
    ("promptsRetired", false, false),
];
const CONTROLS: &[(&str, bool)] = &[
    ("start", true),
    ("capabilities", true),
    ("task.stop", true),
    ("usage.optional", true),
    ("context", true),
    ("sessions.info", false),
    ("sessions.read", false),
    ("history.continue", false),
    ("unknown", false),
];

fn app() -> AppState {
    let mut app = AppState::new("original".into());
    app.session.enable_claude();
    let params = json!({"generation":1,"epoch":4});
    assert!(accepts(&mut app, "claude/conversationReset", &params));
    app
}

fn accepts(app: &mut AppState, method: &str, params: &Value) -> bool {
    app.session
        .accept_claude_fact(method, params.as_object().expect("params"))
}

#[test]
fn every_named_fact_keeps_its_generation_epoch_and_closed_exceptions() {
    for &(name, process, closed) in FACTS {
        let method = format!("claude/{name}");
        for generation in [0, 1, 2] {
            for epoch in [3, 4, 5] {
                for phase in [ClaudePhase::Ready, ClaudePhase::Closed] {
                    let mut app = app();
                    app.session.set_claude_phase_for_test(phase);
                    let expected = generation == 1
                        && (epoch >= 4 || process)
                        && (phase == ClaudePhase::Ready || closed);
                    assert_eq!(
                        accepts(
                            &mut app,
                            &method,
                            &json!({"generation":generation,"epoch":epoch})
                        ),
                        expected,
                        "{name} generation={generation} epoch={epoch} phase={phase:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn every_control_reply_keeps_only_the_five_process_exceptions() {
    for &(request, process) in CONTROLS {
        for generation in [0, 1, 2] {
            for epoch in [3, 4, 5] {
                let mut app = app();
                let params = json!({"generation":generation,"epoch":epoch,"requestMethod":request});
                assert_eq!(
                    accepts(&mut app, "claude/controlReply", &params),
                    generation == 1 && (epoch >= 4 || process),
                    "{params}"
                );
                app.session.set_claude_phase_for_test(ClaudePhase::Closed);
                assert!(!accepts(&mut app, "claude/controlReply", &params));
            }
        }
    }
}

#[test]
fn absent_or_non_numeric_fences_keep_the_existing_permissive_defaults() {
    for value in [Value::Null, json!("1"), json!(-1), json!(true)] {
        let mut app = app();
        assert!(accepts(
            &mut app,
            "claude/historyRecord",
            &json!({"generation":value,"epoch":value})
        ));
    }
    assert!(accepts(&mut app(), "claude/historyRecord", &json!({})));
}

#[test]
fn common_events_drop_old_epochs_except_context_and_fence_present_generation_when_closed() {
    for method in [
        "item/started",
        "item/completed",
        "item/agentMessage/delta",
        "turn/started",
        "turn/completed",
        "thread/tokenUsage/updated",
        "unknown",
    ] {
        for context in [false, true] {
            for generation in [0, 1, 2] {
                for epoch in [3, 4, 5] {
                    let mut app = app();
                    let mut params = json!({"generation":generation,"epoch":epoch});
                    if context {
                        params["claudeContext"] = json!({});
                    }
                    let result = route(&mut app, method, &params);
                    assert_eq!(
                        matches!(result, ClaudeRoute::Handled(None)),
                        generation != 1
                            || (epoch < 4 && !context)
                            || (context && method == "thread/tokenUsage/updated"),
                        "{method} {params}"
                    );
                }
            }
        }
    }
    let mut app = app();
    app.session.close_claude();
    for params in [
        json!({"generation":null}),
        json!({"generation":"bad"}),
        json!({"generation":1}),
    ] {
        assert!(matches!(
            route(&mut app, "turn/started", &params),
            ClaudeRoute::Handled(None)
        ));
    }
    assert!(matches!(
        route(&mut app, "turn/started", &json!({})),
        ClaudeRoute::Continue
    ));
}

#[test]
fn disabled_common_route_does_not_apply_claude_fences() {
    let mut app = AppState::new("original".into());
    let params = json!({"generation":0,"epoch":0,"history":true});
    assert!(matches!(
        route(&mut app, "item/completed", &params),
        ClaudeRoute::Continue
    ));
}

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
            let mut app = AppState::new("original".into());
            app.append_message(carlos_core::session::models::Role::Assistant, "retained");
            let mut params = json!({"generation":generation});
            if let Some(id) = &id {
                params["sessionId"] = id.clone();
            }
            handle(
                &mut app,
                "claude/sessionChanged",
                params.as_object().expect("params"),
            );
            let adopted = generation == 2 && id.as_ref().is_some_and(Value::is_string);
            assert_eq!(
                app.session.thread_id(),
                if adopted {
                    id.as_ref().and_then(Value::as_str).expect("string")
                } else {
                    "original"
                }
            );
            assert_eq!(
                app.session.messages().iter().any(|m| m.text == "retained"),
                !adopted
            );
            assert!(app.session.claude().enabled());
        }
    }
}

#[test]
fn repeated_reset_advances_epoch_without_retiring_the_transcript_again() {
    let mut app = app();
    for epoch in [5, 6] {
        let params = json!({"generation":1,"epoch":epoch,"new_conversation_id":"same"});
        handle(
            &mut app,
            "claude/conversationReset",
            params.as_object().expect("params"),
        );
        if epoch == 5 {
            app.append_message(carlos_core::session::models::Role::Assistant, "after reset");
        }
    }
    assert_eq!(app.session.messages().len(), 1);
    assert_eq!(app.session.messages()[0].text, "after reset");
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
                super::claude_reset::receive(&mut app, &json!({"new_conversation_id":"reset"}));
            }
            let mut facts = json!({});
            if let Some(id) = &id {
                facts["session_id"] = id.clone();
            }
            super::claude_reset::initialize(&mut app, &facts);
            let adopted = waiting && id.as_ref().is_some_and(Value::is_string);
            assert_eq!(
                app.session.thread_id(),
                if adopted {
                    id.as_ref().and_then(Value::as_str).expect("string")
                } else {
                    "original"
                }
            );
            assert_eq!(
                app.session.claude().phase(),
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
    app.session.close_claude();
    assert!(!accepts(
        &mut app,
        "claude/conversationReset",
        &json!({"generation":1,"epoch":9})
    ));
    app.session.set_claude_phase_for_test(ClaudePhase::Ready);
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
        let mut app = AppState::new("original".into());
        handle(&mut app, method, params.as_object().expect("params"));
        assert_eq!(app.session.claude().phase(), phase);
        assert_eq!(app.session.messages()[0].text, row);
        assert_eq!(app.session.status(), status);
    }
}
