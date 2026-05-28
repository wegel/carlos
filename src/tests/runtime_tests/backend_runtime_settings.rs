// Tests for backend-specific startup runtime settings.

fn runtime_test_model(
    model: &str,
    efforts: &[&str],
    default_effort: &str,
    is_default: bool,
) -> ModelInfo {
    ModelInfo {
        model: model.to_string(),
        display_name: model.to_string(),
        supported_efforts: efforts.iter().map(|effort| (*effort).to_string()).collect(),
        default_effort: Some(default_effort.to_string()),
        is_default,
    }
}

#[test]
fn codex_startup_defaults_use_gpt55_high_concise() {
    let defaults = codex_runtime_defaults(None);

    assert_eq!(
        defaults,
        RuntimeDefaults {
            model: Some("gpt-5.5".to_string()),
            effort: Some("high".to_string()),
            summary: Some("concise".to_string()),
        }
    );
}

#[test]
fn claude_startup_defaults_use_opus_xhigh_without_summary() {
    let defaults = claude_runtime_defaults();

    assert_eq!(
        defaults,
        RuntimeDefaults {
            model: Some("opus".to_string()),
            effort: Some("xhigh".to_string()),
            summary: None,
        }
    );
}

#[test]
fn backend_runtime_settings_replace_incompatible_persisted_model() {
    let models = vec![
        runtime_test_model("gpt-5.5", &["low", "medium", "high"], "high", true),
        runtime_test_model("gpt-5.4", &["low", "medium", "high"], "medium", false),
    ];
    let persisted = RuntimeDefaults {
        model: Some("opus".to_string()),
        effort: Some("xhigh".to_string()),
        summary: None,
    };

    let resolved = resolve_backend_runtime_settings(
        ThreadRuntimeSettings::default(),
        &persisted,
        &codex_runtime_defaults(None),
        &models,
        true,
    );

    assert_eq!(resolved.model.as_deref(), Some("gpt-5.5"));
    assert_eq!(resolved.effort.as_deref(), Some("high"));
    assert_eq!(resolved.summary.as_deref(), Some("concise"));
}

#[test]
fn backend_runtime_settings_keep_compatible_model_and_clamp_effort() {
    let models = vec![
        runtime_test_model("gpt-5.5", &["low", "medium", "high"], "high", true),
        runtime_test_model("gpt-5.4", &["low", "medium"], "medium", false),
    ];
    let persisted = RuntimeDefaults {
        model: Some("gpt-5.4".to_string()),
        effort: Some("xhigh".to_string()),
        summary: Some("detailed".to_string()),
    };

    let resolved = resolve_backend_runtime_settings(
        ThreadRuntimeSettings::default(),
        &persisted,
        &codex_runtime_defaults(None),
        &models,
        true,
    );

    assert_eq!(resolved.model.as_deref(), Some("gpt-5.4"));
    assert_eq!(resolved.effort.as_deref(), Some("medium"));
    assert_eq!(resolved.summary.as_deref(), Some("detailed"));
}

#[test]
fn backend_runtime_settings_drop_summary_when_backend_cannot_use_it() {
    let models = vec![runtime_test_model(
        "opus",
        &["low", "medium", "high", "xhigh", "max"],
        "medium",
        true,
    )];
    let persisted = RuntimeDefaults {
        model: Some("gpt-5.5".to_string()),
        effort: Some("high".to_string()),
        summary: Some("concise".to_string()),
    };

    let resolved = resolve_backend_runtime_settings(
        ThreadRuntimeSettings::default(),
        &persisted,
        &claude_runtime_defaults(),
        &models,
        false,
    );

    assert_eq!(resolved.model.as_deref(), Some("opus"));
    assert_eq!(resolved.effort.as_deref(), Some("xhigh"));
    assert_eq!(resolved.summary, None);
}
