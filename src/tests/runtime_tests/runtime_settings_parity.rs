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
