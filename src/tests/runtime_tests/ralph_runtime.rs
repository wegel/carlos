// Tests for ralph-mode runtime state transitions and persistence.

#[test]
fn resolve_initial_runtime_settings_falls_back_to_persisted_defaults() {
    let runtime = carlos_core::protocol_params::ThreadRuntimeSettings {
        model: None,
        effort: None,
        summary: None,
    };
    let defaults = RuntimeDefaults {
        model: Some("gpt-5.4".to_string()),
        effort: Some("high".to_string()),
        summary: Some("concise".to_string()),
    };

    let resolved = resolve_initial_runtime_settings(runtime, &defaults, defaults.summary.clone());

    assert_eq!(resolved.model.as_deref(), Some("gpt-5.4"));
    assert_eq!(resolved.effort.as_deref(), Some("high"));
    assert_eq!(resolved.summary.as_deref(), Some("concise"));
}

#[test]
fn apply_model_settings_returns_defaults_for_persistence() {
    let mut app = AppState::new("thread-1".to_string());
    app.view.runtime_settings.model_settings_model_input = "gpt-5.4".to_string();
    app.view.runtime_settings.model_settings_effort_options = super::state::DEFAULT_EFFORT_OPTIONS
        .iter()
        .map(|value| (*value).to_string())
        .collect();
    app.view.runtime_settings.model_settings_effort_index = 4;
    app.view.runtime_settings.model_settings_summary_options =
        super::state::DEFAULT_SUMMARY_OPTIONS
            .iter()
            .map(|value| (*value).to_string())
            .collect();
    app.view.runtime_settings.model_settings_summary_index = 1;
    app.view.runtime_settings.show_model_settings = true;

    let defaults = app.apply_model_settings();

    assert_eq!(
        defaults,
        RuntimeDefaults {
            model: Some("gpt-5.4".to_string()),
            effort: Some("high".to_string()),
            summary: Some("concise".to_string()),
        }
    );
    assert_eq!(
        app.take_pending_runtime_settings(),
        (
            Some("gpt-5.4".to_string()),
            Some("high".to_string()),
            Some("concise".to_string())
        )
    );
    assert!(!app.view.runtime_settings.show_model_settings);
}

#[test]
fn apply_model_settings_omits_summary_when_backend_does_not_support_it() {
    let mut app = AppState::new("thread-1".to_string());
    app.set_runtime_capabilities(true, false);
    app.view.runtime_settings.model_settings_model_input = "claude-opus-4-6".to_string();
    app.view.runtime_settings.model_settings_effort_options = vec![
        "low".to_string(),
        "medium".to_string(),
        "high".to_string(),
        "max".to_string(),
    ];
    app.view.runtime_settings.model_settings_effort_index = 2;
    app.view.runtime_settings.model_settings_summary_options =
        super::state::DEFAULT_SUMMARY_OPTIONS
            .iter()
            .map(|value| (*value).to_string())
            .collect();
    app.view.runtime_settings.model_settings_summary_index = 1;
    app.view.runtime_settings.show_model_settings = true;

    let defaults = app.apply_model_settings();

    assert_eq!(
        defaults,
        RuntimeDefaults {
            model: Some("claude-opus-4-6".to_string()),
            effort: Some("high".to_string()),
            summary: None,
        }
    );
    assert_eq!(
        app.take_pending_runtime_settings(),
        (
            Some("claude-opus-4-6".to_string()),
            Some("high".to_string()),
            None
        )
    );
}
