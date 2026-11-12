//! Enumerate native model choices and local editor saves against the previous app rules.

use serde_json::{json, Value};

use crate::app::claude_models::{
    apply_capabilities, observe_model, prepare_saved_choices, prompt_choices,
};
use crate::app::claude_state::ClaudePhase;
use crate::app::state::AppState;
use crate::session::runtime_settings::RuntimeDefaults;

fn app(model: Option<&str>, effort: Option<&str>) -> AppState {
    let mut app = AppState::new("root".into());
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

fn choices(app: &AppState) -> Value {
    let settings = app.session.runtime_settings();
    let explicit = prompt_choices(app, Some("chosen".into()), Some("effort".into()));
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
    let cases = [
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
    ];
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
        app.session.set_claude_observed_model_for_test(model);
    }
    apply_capabilities(&mut app, &metadata());
    assert_eq!(choices(&app), expected, "{model:?} {effort:?} {observed:?}");
    assert_eq!(app.session.status(), "Claude ready");
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
    let models = app.session.runtime_settings().available_models();
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
            app.session.runtime_settings().available_models()[0].supported_efforts,
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
        app.session.set_claude_phase(phase);
        app.claude.awaiting_reset_init = awaiting;
        apply_capabilities(&mut app, &metadata());
        assert_eq!(app.session.claude().phase() as u8, expected as u8);
        assert_eq!(
            app.session.claude().capabilities().is_none(),
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
        observe_model(&mut app, "old");
        app.session
            .set_context_usage(Some(crate::session::context_usage::ContextUsage {
                used: 10,
                max: 100,
            }));
        let revision = app.session.claude().activity().context_revision();
        observe_model(&mut app, "fast-2026");
        assert_eq!(
            app.session.claude().activity().context_revision(),
            revision + 1
        );
        assert!(app.session.context_usage().is_none());
        let model = app
            .session
            .runtime_settings()
            .current_model()
            .as_deref()
            .unwrap_or("none");
        // The first observation pins the inherited model until a switch, just as before.
        assert_eq!(model, expected);
        observe_model(&mut app, "fast-2026");
        assert_eq!(
            app.session.claude().activity().context_revision(),
            revision + 1
        );
    }
}

#[test]
fn editor_saves_preserve_inheritance_only_for_unchanged_model_and_nonexplicit_effort() {
    for (editing, inherited, model, effort, expected) in [
        (
            false,
            true,
            "fast",
            "high",
            json!(["fast", "high", [null, null], "fast", "high"]),
        ),
        (
            true,
            true,
            "fast",
            "high",
            json!([null, null, [null, null], null, null]),
        ),
        (
            true,
            true,
            "small",
            "low",
            json!(["small", "low", ["chosen", "effort"], "small", "low"]),
        ),
        (
            true,
            false,
            "fast",
            "auto",
            json!(["fast", null, ["chosen", null], "fast", null]),
        ),
        (
            true,
            false,
            "fast",
            "high",
            json!(["fast", "high", ["chosen", "effort"], "fast", "high"]),
        ),
    ] {
        let mut app = app(Some("fast"), Some("high"));
        app.session
            .set_claude_inheritance_for_test(inherited, inherited);
        app.view.claude_settings_edit = editing.then(|| ("fast".into(), "high".into()));
        app.queue_runtime_settings(Some(model.into()), Some(effort.into()), None);
        let mut defaults = saved_defaults(model, effort);
        prepare_saved_choices(&mut app, &mut defaults);
        assert_eq!(
            json!([
                defaults.model,
                defaults.effort,
                prompt_choices(&app, Some("chosen".into()), Some("effort".into())),
                app.session.runtime_settings().pending_model(),
                app.session.runtime_settings().pending_effort()
            ]),
            expected
        );
        assert_eq!(defaults.summary.as_deref(), Some("keep"));
    }
}

#[test]
fn switching_clears_only_inherited_current_choices_without_pending_overrides() {
    for (inherited, pending, expected) in [
        (true, false, json!([null, null, null, null, [null, null]])),
        (
            true,
            true,
            json!(["fast", "high", "small", "low", [null, null]]),
        ),
        (
            false,
            false,
            json!(["fast", "high", null, null, ["chosen", "effort"]]),
        ),
        (
            false,
            true,
            json!(["fast", "high", "small", "low", ["chosen", "effort"]]),
        ),
    ] {
        let mut app = app(Some("fast"), Some("high"));
        app.session
            .set_claude_inheritance_for_test(inherited, inherited);
        app.view.claude_settings_edit = Some(("local".into(), "snapshot".into()));
        if pending {
            app.queue_runtime_settings(Some("small".into()), Some("low".into()), None);
        }
        crate::app::claude_session_history::switch_session(&mut app, 2, "branch");
        assert_eq!(choices(&app), expected);
        assert_eq!(
            app.view.claude_settings_edit,
            Some(("local".into(), "snapshot".into()))
        );
        assert_eq!(
            app.session.claude().phase() as u8,
            ClaudePhase::Starting as u8
        );
        assert!(app.session.claude().capabilities().is_none());
    }
}

fn saved_defaults(model: &str, effort: &str) -> RuntimeDefaults {
    RuntimeDefaults {
        model: Some(model.into()),
        effort: Some(effort.into()),
        summary: Some("keep".into()),
    }
}

#[test]
fn launch_omits_only_missing_model_and_missing_or_auto_effort() {
    for (model, effort, expected) in [
        (None, None, json!([null, null])),
        (Some("fast"), None, json!(["m", null])),
        (None, Some("high"), json!([null, "e"])),
        (Some("fast"), Some("auto"), json!(["m", null])),
        (Some("fast"), Some("high"), json!(["m", "e"])),
    ] {
        let mut app = app(model, effort);
        app.session.start_claude_settings();
        assert_eq!(
            app.session.claude().phase() as u8,
            ClaudePhase::Starting as u8
        );
        assert_eq!(
            json!(prompt_choices(&app, Some("m".into()), Some("e".into()))),
            expected
        );
    }
}

#[test]
fn model_and_effort_inheritance_filter_prompt_choices_independently() {
    for (model, effort, expected) in [
        (true, true, json!([null, null])),
        (false, true, json!(["m", null])),
        (true, false, json!([null, "e"])),
        (false, false, json!(["m", "e"])),
    ] {
        let mut app = app(None, None);
        app.session.set_claude_inheritance_for_test(model, effort);
        assert_eq!(
            json!(prompt_choices(&app, Some("m".into()), Some("e".into()))),
            expected
        );
    }
}
