//! Reduce startup callbacks and model/context answers at the actual native boundary.

use serde_json::json;

use super::{frame, sdk};
use crate::claude_backend::FixtureNormalizer;
use crate::session::state::SessionState;

#[test]
fn startup_callback_remains_visible_while_the_sdk_initializes() {
    let mut app = SessionState::new("session".into(), Default::default(), false);
    let mut native = FixtureNormalizer::default();
    app.set_claude_phase_for_test(crate::session::claude::ClaudePhase::Starting);
    frame(
        &mut app,
        &mut native,
        json!({ "v": 1, "type": "callback", "kind": "permission", "callbackId": "startup",
        "payload": { "title": "Allow startup?", "toolName": "Bash", "input": { "command": "pwd" } } }),
        None,
    );
    assert_eq!(
        app.approval()
            .pending()
            .expect("startup question")
            .facts()
            .title,
        "Allow startup?"
    );
    assert!(app.claude().phase() != crate::session::claude::ClaudePhase::Ready);
    frame(
        &mut app,
        &mut native,
        json!({ "v": 1, "type": "reply", "ok": true, "result": { "models": [] } }),
        Some("start"),
    );
    assert!(app.claude().phase() == crate::session::claude::ClaudePhase::Ready);
    assert!(app.approval().pending().is_some());
}

#[test]
fn callback_cancellation_promotes_the_next_native_request_without_losing_it() {
    let mut app = SessionState::new("session".into(), Default::default(), false);
    let mut native = FixtureNormalizer::default();
    for id in ["first", "second"] {
        frame(
            &mut app,
            &mut native,
            json!({ "v": 1, "type": "callback", "kind": "permission", "callbackId": id,
            "payload": { "toolName": "Read", "input": {} } }),
            None,
        );
    }
    frame(
        &mut app,
        &mut native,
        json!({ "v": 1, "type": "event", "event": "callback_cancelled", "callbackId": "first" }),
        None,
    );
    assert_eq!(
        app.approval()
            .pending()
            .expect("second request")
            .facts()
            .request_id,
        json!("second")
    );
}

#[test]
fn models_and_context_come_from_native_answers_without_guessing() {
    let mut app = SessionState::new("session".into(), Default::default(), false);
    let mut native = FixtureNormalizer::default();
    frame(
        &mut app,
        &mut native,
        json!({ "v": 1, "type": "reply", "ok": true, "result": { "models": [
        { "value": "future-model", "displayName": "Future", "supportedEffortLevels": ["high", "max"] }
    ] } }),
        Some("start"),
    );
    let runtime = app.runtime_settings();
    assert_eq!(runtime.available_models()[0].model, "future-model");
    assert_eq!(
        runtime.available_models()[0].supported_efforts,
        ["high", "max"]
    );
    frame(
        &mut app,
        &mut native,
        json!({ "v": 1, "type": "reply", "ok": true,
        "result": { "totalTokens": 123, "rawMaxTokens": 456 } }),
        Some("context"),
    );
    assert_eq!(app.context_usage().expect("native context").used, 123);
    assert_eq!(app.context_usage().expect("native context").max, 456);
}

#[test]
fn initial_native_model_observation_does_not_overwrite_a_carlos_choice() {
    for selected in [None, Some("saved-model".to_string())] {
        let mut app = SessionState::new("session".into(), Default::default(), false);
        let mut native = FixtureNormalizer::default();
        app.set_runtime_settings(selected.clone(), Some("high".to_string()), None);
        sdk(
            &mut app,
            &mut native,
            json!({ "type": "system", "subtype": "init", "session_id": "session", "model": "native-default" }),
        );
        let runtime = app.runtime_settings();
        assert_eq!(
            runtime.current_model(),
            &selected.or_else(|| Some("native-default".to_string()))
        );
        assert_eq!(runtime.current_effort().as_deref(), Some("high"));
    }
}

#[test]
fn native_model_metadata_rejects_saved_other_backend_choices_in_either_startup_order() {
    for init_first in [true, false] {
        let mut app = SessionState::new("session".into(), Default::default(), false);
        let mut native = FixtureNormalizer::default();
        app.set_runtime_settings(Some("gpt-5.5".to_string()), Some("xhigh".to_string()), None);
        let init = json!({ "type": "system", "subtype": "init", "session_id": "session", "model": "claude-native" });
        let capabilities = json!({ "v": 1, "type": "reply", "ok": true, "result": { "models": [
            { "value": "sonnet", "resolvedModel": "claude-native", "supportedEffortLevels": ["low", "high"] }
        ] } });
        if init_first {
            sdk(&mut app, &mut native, init.clone());
        }
        frame(&mut app, &mut native, capabilities, Some("start"));
        if !init_first {
            sdk(&mut app, &mut native, init);
        }
        let runtime = app.runtime_settings();
        assert_eq!(runtime.current_model().as_deref(), Some("sonnet"));
        assert!(runtime.current_effort().is_none());
    }
}

#[test]
fn native_resolved_model_metadata_keeps_a_saved_canonical_id_selectable() {
    let mut app = SessionState::new("session".into(), Default::default(), false);
    let mut native = FixtureNormalizer::default();
    app.set_runtime_settings(
        Some("claude-opus-current".to_string()),
        Some("max".to_string()),
        None,
    );
    frame(
        &mut app,
        &mut native,
        json!({ "v": 1, "type": "reply", "ok": true, "result": { "models": [
        { "value": "opus", "resolvedModel": "claude-opus-current", "supportedEffortLevels": ["high", "max"] }
    ] } }),
        Some("start"),
    );
    let runtime = app.runtime_settings();
    assert_eq!(
        runtime.current_model().as_deref(),
        Some("claude-opus-current")
    );
    assert_eq!(runtime.current_effort().as_deref(), Some("max"));
    assert_eq!(runtime.available_models()[0].model, "claude-opus-current");
}

#[test]
fn undeclared_dialog_error_does_not_disable_live_controls() {
    let mut app = SessionState::new("session".into(), Default::default(), false);
    let mut native = FixtureNormalizer::default();
    frame(
        &mut app,
        &mut native,
        json!({ "v": 1, "type": "event", "event": "error", "callbackId": "unsupported",
        "dialogKind": "future", "error": { "code": "unsupported_dialog", "message": "Cannot render future dialog" } }),
        None,
    );
    assert!(app.claude().phase() == crate::session::claude::ClaudePhase::Ready);
    assert!(app.approval().pending().is_none());
    let error = &app.messages().last().expect("visible error").text;
    assert!(error.contains("Cannot render future dialog"));
    assert!(error.contains("request unsupported, kind future"));
}
