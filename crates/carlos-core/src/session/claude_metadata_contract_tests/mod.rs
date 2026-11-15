//! Preserve native Claude capability and inheritance matrices without a terminal owner.

use crate::session::claude::ClaudePhase;
use serde_json::{json, Value};

use crate::session::state::SessionState;

mod inheritance;

/// Seed the original startup choices directly on the native session.
pub(super) fn app(model: Option<&str>, effort: Option<&str>) -> SessionState {
    let mut app = SessionState::new("root".into(), Default::default(), false);
    app.set_runtime_settings(model.map(str::to_string), effort.map(str::to_string), None);
    app
}

fn metadata() -> Value {
    json!({"models":[
        {"value":"fast","resolvedModel":"fast-2026","displayName":"Fast",
         "supportsEffort":true,"supportedEffortLevels":["low","high"]},
        {"value":"small","supportsEffort":false,"supportedEffortLevels":["high"]},
        {"displayName":"missing value"}
    ]})
}

/// Compare current, pending and inherited values from the same Core session.
pub(super) fn choices(app: &SessionState) -> Value {
    let settings = app.runtime_settings();
    let explicit = app.claude_prompt_choices(Some("chosen".into()), Some("effort".into()));
    json!([
        settings.current_model(),
        settings.current_effort(),
        settings.pending_model(),
        settings.pending_effort(),
        explicit
    ])
}

#[test]
fn native_metadata_preserves_supported_choices_and_rejects_only_known_bad_efforts() {
    let cases = requested_metadata_cases()
        .into_iter()
        .chain(observed_metadata_cases());
    for (model, effort, observed, expected) in cases {
        assert_metadata(model, effort, observed, expected);
    }
}

fn assert_metadata(
    model: Option<&str>,
    effort: Option<&str>,
    observed: Option<&str>,
    expected: Value,
) {
    let mut app = app(model, effort);
    if let Some(model) = observed {
        app.set_claude_observed_model_for_test(model);
    }
    apply_capabilities(&mut app, &metadata());
    assert_eq!(choices(&app), expected, "{model:?} {effort:?} {observed:?}");
    assert_eq!(app.status(), "Claude ready");
}

#[test]
fn pending_model_precedes_current_and_resolved_identifiers_keep_their_catalog_row() {
    let mut app = app(Some("gone"), Some("max"));
    app.queue_runtime_settings(Some("fast-2026".into()), Some("low".into()), None);
    apply_capabilities(&mut app, &metadata());
    assert_eq!(
        choices(&app),
        json!(["fast-2026", "max", "fast-2026", "low", ["chosen", null]])
    );
    let models = app.runtime_settings().available_models();
    assert_eq!(models.len(), 2);
    assert_eq!(models[0].model, "fast-2026");
    assert_eq!(models[0].display_name, "Fast");
    assert_eq!(models[0].supported_efforts, ["low", "high"]);
    assert_eq!(models[1].supported_efforts, ["auto"]);
}

#[test]
fn malformed_effort_lists_keep_only_string_levels_and_fall_back_to_auto() {
    for (model, expected) in [
        (json!({"value":"m"}), vec!["auto"]),
        (
            json!({"value":"m","supportedEffortLevels":[]}),
            vec!["auto"],
        ),
        (
            json!({"value":"m","supportedEffortLevels":[null,3,"high"]}),
            vec!["high"],
        ),
        (
            json!({"value":"m","supportsEffort":false,"supportedEffortLevels":["high"]}),
            vec!["auto"],
        ),
    ] {
        let mut app = app(None, None);
        apply_capabilities(&mut app, &json!({"models":[model]}));
        assert_eq!(
            app.runtime_settings().available_models()[0].supported_efforts,
            expected
        );
    }
}

#[test]
fn capabilities_ignore_closed_helpers_and_do_not_skip_the_reset_init_guard() {
    for (phase, awaiting, expected) in [
        (ClaudePhase::Closed, false, ClaudePhase::Closed),
        (ClaudePhase::Starting, false, ClaudePhase::Ready),
        (ClaudePhase::Starting, true, ClaudePhase::Starting),
        (ClaudePhase::Ready, true, ClaudePhase::Ready),
    ] {
        let mut app = app(Some("fast"), Some("high"));
        app.set_claude_phase_for_test(phase);
        app.set_claude_awaiting_init_for_test(awaiting);
        apply_capabilities(&mut app, &metadata());
        assert_eq!(app.claude().phase() as u8, expected as u8);
        assert_eq!(
            app.claude().capabilities().is_none(),
            phase == ClaudePhase::Closed
        );
    }
}

#[test]
fn observing_a_changed_model_fences_context_without_overwriting_explicit_choices() {
    for (current, pending, expected) in [
        (None, None, "old"),
        (Some("small"), None, "small"),
        (None, Some("small"), "none"),
    ] {
        let mut app = app(current, None);
        apply_capabilities(&mut app, &metadata());
        app.queue_runtime_settings(pending.map(str::to_string), None, None);
        app.observe_claude_model("old");
        app.set_context_usage(Some(crate::session::context_usage::ContextUsage {
            used: 10,
            max: 100,
        }));
        let revision = app.claude().activity().context_revision();
        app.observe_claude_model("fast-2026");
        assert_eq!(app.claude().activity().context_revision(), revision + 1);
        assert!(app.context_usage().is_none());
        let model = app
            .runtime_settings()
            .current_model()
            .as_deref()
            .unwrap_or("none");
        // The first observation pins the inherited model until a switch, just as before.
        assert_eq!(model, expected);
        app.observe_claude_model("fast-2026");
        assert_eq!(app.claude().activity().context_revision(), revision + 1);
    }
}

fn apply_capabilities(core: &mut SessionState, capabilities: &Value) {
    core.receive_claude_capabilities(capabilities, &mut Default::default());
}

type MetadataCase = (
    Option<&'static str>,
    Option<&'static str>,
    Option<&'static str>,
    Value,
);

fn requested_metadata_cases() -> [MetadataCase; 5] {
    [
        (
            None,
            None,
            None,
            json!([null, null, null, null, [null, null]]),
        ),
        (
            Some("fast"),
            Some("high"),
            None,
            json!(["fast", "high", null, null, ["chosen", null]]),
        ),
        (
            Some("fast"),
            Some("max"),
            None,
            json!(["fast", null, null, null, ["chosen", null]]),
        ),
        (
            Some("small"),
            Some("high"),
            None,
            json!(["small", null, null, null, ["chosen", null]]),
        ),
        (
            Some("gone"),
            Some("high"),
            None,
            json!([null, null, null, null, [null, null]]),
        ),
    ]
}

fn observed_metadata_cases() -> [MetadataCase; 3] {
    [
        (
            None,
            None,
            Some("fast-2026"),
            json!(["fast", null, null, null, [null, null]]),
        ),
        (
            Some("custom"),
            Some("high"),
            Some("custom"),
            json!(["custom", "high", null, null, [null, null]]),
        ),
        (
            Some("fast-2026"),
            Some("auto"),
            None,
            json!(["fast-2026", "auto", null, null, ["chosen", null]]),
        ),
    ]
}
