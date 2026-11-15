// Enumerate settings rules that must survive moving backend choices into the session.

fn settings_values(values: [Option<&str>; 3]) -> (Option<String>, Option<String>, Option<String>) {
    (
        values[0].map(str::to_string),
        values[1].map(str::to_string),
        values[2].map(str::to_string),
    )
}

fn seed_runtime_choices() -> AppState {
    let mut app = AppState::new("settings".into());
    app.set_runtime_settings(
        Some("old".into()),
        Some("high".into()),
        Some("concise".into()),
    );
    app
}

#[test]
fn settings_editor_uses_pending_choices_and_stable_default_first_model_order() {
    let mut app = seed_runtime_choices();
    app.set_available_models(vec![
        runtime_test_model("other", &["low", "high"], "high", false),
        runtime_test_model("first", &["low", "high"], "low", true),
        runtime_test_model("second", &["low", "high"], "high", true),
        runtime_test_model("last", &["low", "high"], "low", false),
    ]);
    app.open_model_settings();
    assert_eq!(app.model_settings_model_value(), "first");
    for expected in ["second", "other", "last", "first"] {
        app.model_settings_cycle_model(1);
        assert_eq!(app.model_settings_model_value(), expected);
    }
    app.queue_runtime_settings(
        Some("second".into()),
        Some("LOW".into()),
        Some("none".into()),
    );
    app.open_model_settings();
    assert_eq!(app.model_settings_model_value(), "second");
    assert_eq!(app.model_settings_effort_value(), "low");
    assert_eq!(app.model_settings_summary_value(), "none");
    let defaults = app.apply_model_settings();
    assert_eq!(
        defaults,
        RuntimeDefaults {
            model: Some("second".into()),
            effort: Some("low".into()),
            summary: Some("none".into())
        }
    );
}

#[test]
fn settings_editor_effort_fallbacks_follow_requested_default_then_bounded_medium() {
    for (options, default, requested, expected) in [
        (vec!["low", "HIGH"], "low", "high", "HIGH"),
        (vec!["low", "high"], "high", "unknown", "high"),
        (vec!["low", "high"], "missing", "unknown", "high"),
        (vec![], "missing", "unknown", "medium"),
        (vec!["only"], "missing", "unknown", "only"),
    ] {
        let mut app = AppState::new("effort".into());
        app.set_available_models(vec![runtime_test_model("model", &options, default, true)]);
        app.queue_runtime_settings(
            Some("model".into()),
            Some(requested.into()),
            Some("unknown".into()),
        );
        app.open_model_settings();
        assert_eq!(app.model_settings_effort_value(), expected);
        assert_eq!(app.model_settings_summary_value(), "auto");
    }
}

#[test]
fn settings_editor_edits_and_cancel_leave_session_choices_untouched() {
    let mut app = seed_runtime_choices();
    let before = app.next_turn_runtime_settings();
    app.open_model_settings();
    assert_eq!(app.model_settings_model_value(), "old");
    app.model_settings_insert_char('é');
    assert_eq!(app.model_settings_model_value(), "oldé");
    app.model_settings_backspace();
    app.model_settings_cycle_model(-1);
    assert_eq!(app.model_settings_model_value(), "old");
    app.model_settings_cycle_effort(-1);
    assert_eq!(app.model_settings_effort_value(), "medium");
    app.model_settings_cycle_summary(-1);
    assert_eq!(app.model_settings_summary_value(), "auto");
    app.model_settings_move_field(false);
    app.close_model_settings();
    assert_eq!(app.next_turn_runtime_settings(), before);
    assert_eq!(app.take_pending_runtime_settings(), (None, None, None));
}

#[test]
fn settings_launch_override_keeps_empty_and_spaced_native_strings() {
    for (requested, expected) in [
        (None, None),
        (Some(""), Some("")),
        (Some(" spaced "), Some(" spaced ")),
    ] {
        let mut app = seed_runtime_choices();
        let response = json!({"result":{"thread":{"id":"settings"},"model":"old","reasoningEffort":"high","reasoningSummary":"concise"}}).to_string();
        crate::app::codex_settings_draft::initialize(
            &mut app,
            &response,
            true,
            requested.map(str::to_string),
        )
        .expect("native settings");
        assert_eq!(app.take_pending_runtime_settings().2.as_deref(), expected);
    }
}

#[test]
fn settings_constructor_receives_startup_choices_without_loading_files() {
    for input in [None, Some("  "), Some(" chosen ")] {
        let preferences = RuntimeDefaults {
            model: input.map(str::to_string),
            effort: input.map(str::to_string),
            summary: input.map(str::to_string),
        };
        let app = AppState::with_runtime_defaults("startup".into(), preferences.clone());
        assert_eq!(app.next_turn_runtime_settings(), settings_values([None; 3]));
        assert_eq!(
            app.session.runtime_settings().startup_preferences(),
            &preferences
        );
    }
}
