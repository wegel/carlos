//! Preserve exact helper generation, explicit settings and native source anchors.

use serde_json::{json, Value};

use super::{core, frame, switch};
use crate::claude_backend::FixtureNormalizer;
use crate::session::claude::ClaudePhase;
use crate::session::runtime_settings::RuntimeSetting;

#[test]
fn old_generation_close_cancel_and_metadata_cannot_touch_the_new_session() {
    let mut app = core("original");
    let mut native = FixtureNormalizer::default();
    switch(&mut app, &mut native);
    frame(
        &mut app,
        &mut native,
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
        frame(&mut app, &mut native, 1, event, Some("start"));
    }
    assert_eq!(app.thread_id(), "new-session");
    assert!(app.claude().phase() == ClaudePhase::Starting);
    assert!(app.claude().capabilities().is_none());
    assert_eq!(
        app.approval()
            .pending()
            .expect("new callback remains")
            .facts()
            .request_id,
        "same-native-id"
    );
    assert_eq!(app.export_approvals().len(), 1);
}

#[test]
fn session_switch_preserves_explicit_choices_and_releases_inherited_observations() {
    for explicit in [false, true] {
        let mut app = core("original");
        let mut native = FixtureNormalizer::default();
        app.record_runtime_choice(RuntimeSetting::Model, Some("sdk-opus".to_string()));
        app.record_runtime_choice(RuntimeSetting::Effort, Some("high".to_string()));
        app.set_claude_inheritance_for_test(!explicit, !explicit);
        switch(&mut app, &mut native);
        assert_eq!(
            app.runtime_settings().current_model().as_deref(),
            explicit.then_some("sdk-opus")
        );
        assert_eq!(
            app.runtime_settings().current_effort().as_deref(),
            explicit.then_some("high")
        );
        assert_eq!(app.claude().inherits_model(), !explicit);
    }
}

#[test]
fn loaded_records_keep_sources_uuid_anchors_and_unknown_metadata_separate() {
    let mut app = core("session");
    let mut native = FixtureNormalizer::default();
    let rows: Vec<Value> = ["human", "automatic", "unknown", "system"].iter().enumerate().map(|(index, source)| {
        json!({"source":source,"message":{"type":"user","uuid":format!("uuid-{index}"),"session_id":"session",
            "futureMetadata":{"retained":true}, "message":{"role":"user","content":format!("text {index}")}}})
    }).collect();
    frame(
        &mut app,
        &mut native,
        1,
        json!({"type":"reply","ok":true,"result":{"messages":rows,"nextOffset":null}}),
        Some("sessions.read"),
    );
    assert_eq!(app.input_history().text(0), "text 0");
    let entry = app
        .input_history()
        .entry_id(0)
        .expect("native human anchor");
    assert_eq!(app.input_history().turn_id(entry), Some("uuid-0"));
    assert_eq!(app.claude_history().unknown().len(), 1);
    assert_eq!(app.claude_history().unknown()[0].uuid(), "uuid-2");
    assert_eq!(app.claude_history().records().len(), 4);
    assert_eq!(
        app.claude_history().records()[3]["futureMetadata"]["retained"],
        true
    );
}
