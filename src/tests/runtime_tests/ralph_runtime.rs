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
