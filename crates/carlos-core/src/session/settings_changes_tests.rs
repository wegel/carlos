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
