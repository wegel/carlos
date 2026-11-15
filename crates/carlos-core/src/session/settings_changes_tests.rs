//! Preserve omitted settings and validate the complete intent before touching accepted choices.

use carlos_proto::session::Backend;
use carlos_proto::settings::{ModelInfo, SettingValue, SettingsChange};

use super::runtime_settings::RuntimeDefaults;
use super::state::SessionState;

#[test]
fn partial_intent_captures_defaults_without_inventing_unknown_fields() {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.set_runtime_settings(Some("native".into()), Some("low".into()), None);
    let intent = session
        .prepare_settings_change(
            &session.settings_revision(),
            SettingsChange {
                effort: Some(SettingValue::Value("high".into())),
                ..Default::default()
            },
        )
        .expect("explicit effort change");
    assert_eq!(intent.native, serde_json::json!({"effort":"high"}));
    assert_eq!(
        intent.defaults,
        RuntimeDefaults {
            model: Some("native".into()),
            effort: Some("high".into()),
            summary: None
        }
    );
    assert_eq!(
        session.runtime_settings().current_effort().as_deref(),
        Some("low")
    );
    session.set_runtime_settings(Some("newer".into()), None, None);
    assert_eq!(intent.defaults.model.as_deref(), Some("native"));
}

#[test]
fn invalid_effort_or_changed_catalog_cannot_partially_accept_a_model() {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.set_available_models(vec![ModelInfo {
        model: "chosen".into(),
        display_name: "Chosen".into(),
        supported_efforts: vec!["low".into()],
        default_effort: None,
        is_default: true,
    }]);
    let expected = session.settings_revision();
    let change = SettingsChange {
        model: Some(SettingValue::Value("chosen".into())),
        effort: Some(SettingValue::Value("impossible".into())),
        ..Default::default()
    };
    assert!(session.prepare_settings_change(&expected, change).is_err());
    assert!(session.runtime_settings().current_model().is_none());
    session.set_available_models(Vec::new());
    assert!(session
        .prepare_settings_change(&expected, SettingsChange::default())
        .is_err());
}

#[test]
fn appearance_only_claude_defaults_keep_observed_choices_inherited() {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.set_client_identity(Backend::Claude, "host".into());
    session.start_claude_settings();
    session.observe_claude_model("observed-native");
    let intent = session
        .prepare_settings_change(&session.settings_revision(), SettingsChange::default())
        .expect("unchanged choices");
    assert_eq!(intent.defaults, RuntimeDefaults::default());
    assert_eq!(intent.native, serde_json::json!({}));
}

#[test]
fn claude_inheritance_is_an_explicit_native_edit_and_changes_same_value_revision() {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.set_client_identity(Backend::Claude, "host".into());
    session.start_claude_settings();
    session.observe_claude_model("observed");
    let inherited = session.settings_revision();
    session.accept_claude_settings(crate::backend_commands::SettingsPatch {
        model: Some(Some("observed".into())),
        effort: None,
    });
    assert_ne!(inherited, session.settings_revision());
    assert_eq!(
        session.settings_defaults().model.as_deref(),
        Some("observed")
    );
    let intent = session
        .prepare_settings_change(
            &session.settings_revision(),
            SettingsChange {
                model: Some(SettingValue::Inherit),
                effort: Some(SettingValue::Inherit),
                summary: None,
            },
        )
        .expect("inherit native settings");
    assert_eq!(
        intent.native,
        serde_json::json!({"model":null,"effort":null})
    );
    assert_eq!(intent.defaults.model, None);
    assert_eq!(intent.defaults.effort, None);
}

#[test]
fn models_without_effort_clear_captured_defaults_without_pinning_omitted_native_fields() {
    for (backend, efforts, expected) in [
        (
            Backend::Codex,
            Vec::new(),
            serde_json::json!({"model":"chosen"}),
        ),
        (
            Backend::Grok,
            Vec::new(),
            serde_json::json!({"model":"chosen"}),
        ),
        (
            Backend::Claude,
            vec!["auto".into()],
            serde_json::json!({"model":"chosen","effort":null}),
        ),
    ] {
        let mut session = SessionState::new("root".into(), Default::default(), false);
        session.set_client_identity(backend, "host".into());
        session.set_runtime_settings(Some("previous".into()), Some("high".into()), None);
        session.set_available_models(vec![ModelInfo {
            model: "chosen".into(),
            display_name: "Chosen".into(),
            supported_efforts: efforts,
            default_effort: None,
            is_default: true,
        }]);
        let intent = session
            .prepare_settings_change(
                &session.settings_revision(),
                SettingsChange {
                    model: Some(SettingValue::Value("chosen".into())),
                    ..Default::default()
                },
            )
            .expect("model-only change");
        assert_eq!(intent.defaults.effort, None);
        assert_eq!(intent.native, expected);
    }
}
