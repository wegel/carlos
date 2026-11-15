//! Enumerate generation, epoch, closed, reset, and session-adoption gates.

use crate::session::claude::ClaudePhase;
use serde_json::{json, Value};

use crate::session::state::SessionState;

mod adoption;

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

/// Seed generation one at epoch four without a terminal view.
pub(super) fn app() -> SessionState {
    let mut app = SessionState::new("original".into(), Default::default(), false);
    app.enable_claude();
    let params = json!({"generation":1,"epoch":4});
    assert!(accepts(&mut app, "claude/conversationReset", &params));
    app
}

/// Ask the actual Core generation and epoch gate about one named fact.
pub(super) fn accepts(app: &mut SessionState, method: &str, params: &Value) -> bool {
    app.accept_claude_fact(method, params.as_object().expect("params"))
}

#[test]
fn every_named_fact_keeps_its_generation_epoch_and_closed_exceptions() {
    for &(name, process, closed) in FACTS {
        let method = format!("claude/{name}");
        for generation in [0, 1, 2] {
            for epoch in [3, 4, 5] {
                for phase in [ClaudePhase::Ready, ClaudePhase::Closed] {
                    let mut app = app();
                    app.set_claude_phase_for_test(phase);
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
                app.set_claude_phase_for_test(ClaudePhase::Closed);
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
                    let result = app.receive_claude_message(method, &params);
                    assert_eq!(
                        result.is_some_and(|update| update.action.is_none()),
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
    app.close_claude();
    for params in [
        json!({"generation":null}),
        json!({"generation":"bad"}),
        json!({"generation":1}),
    ] {
        assert!(app
            .receive_claude_message("turn/started", &params)
            .is_some_and(|update| update.action.is_none()));
    }
    assert!(app
        .receive_claude_message("turn/started", &json!({}))
        .is_none());
}

#[test]
fn disabled_common_route_does_not_apply_claude_fences() {
    let mut app = SessionState::new("original".into(), Default::default(), false);
    let params = json!({"generation":0,"epoch":0,"history":true});
    assert!(app
        .receive_claude_message("item/completed", &params)
        .is_none());
}
