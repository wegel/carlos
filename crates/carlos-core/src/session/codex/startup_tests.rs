//! Keep new, resumed and reconnect choices distinct from observed native values.

use serde_json::json;

use crate::session::model_info::ModelInfo;
use crate::session::runtime_settings::RuntimeDefaults;
use crate::session::state::SessionState;

#[test]
fn new_session_queues_only_compatible_saved_differences_without_faking_native_facts() {
    let mut session = session(false, choices("saved", "MAX", "detailed"), None);
    session.install_codex_startup();
    assert_eq!(
        session.next_turn_runtime_settings(),
        (
            Some("saved".into()),
            Some("max".into()),
            Some("native-summary".into())
        )
    );
    assert_eq!(
        session.runtime_settings().current_model().as_deref(),
        Some("native")
    );
    assert_eq!(
        session.runtime_settings().current_effort().as_deref(),
        Some("low")
    );
    assert_eq!(
        session.codex_settings().future(),
        &json!({"model":"native","effort":"low","summary":"native-summary"})
    );
    assert_eq!(
        session.codex_connection.initial_choices,
        Some(json!({"model":"saved","effort":"max"}))
    );
}

#[test]
fn new_session_keeps_native_choices_and_fills_only_missing_summary() {
    let mut session = session(false, RuntimeDefaults::default(), None);
    session.install_codex_startup();
    assert_eq!(session.codex_connection.initial_choices, None);
    session.prepare_codex_startup(false, None);
    session.set_runtime_settings(Some("native".into()), Some("low".into()), None);
    session.set_codex_future_settings(session.codex_runtime_choices());
    session.install_codex_startup();
    assert_eq!(
        session.codex_connection.initial_choices,
        Some(json!({"summary":"concise"}))
    );
    assert_eq!(session.runtime_settings().current_summary(), &None);
}

#[test]
fn unsupported_saved_model_and_effort_use_the_existing_catalog_fallbacks() {
    let mut session = session(false, choices("missing", "ultra", "detailed"), None);
    session.install_codex_startup();
    assert_eq!(
        session.codex_connection.initial_choices,
        Some(json!({"effort":"high"}))
    );
    assert_eq!(session.runtime_settings().pending_model(), &None);
    assert_eq!(
        session.runtime_settings().pending_effort().as_deref(),
        Some("high")
    );
}

#[test]
fn models_without_effort_never_receive_a_synthetic_effort_request() {
    let mut session = session(false, choices("saved", "ultra", "detailed"), None);
    session.set_available_models(vec![
        model("native", &["low", "high"], true),
        model("saved", &[], false),
    ]);
    session.install_codex_startup();
    assert_eq!(
        session.codex_connection.initial_choices,
        Some(json!({"model":"saved"}))
    );
    assert_eq!(session.runtime_settings().pending_effort(), &None);
}

#[test]
fn resumed_session_keeps_native_unknowns_and_applies_only_the_explicit_summary_override() {
    let mut session = session(true, choices("saved", "max", "detailed"), None);
    session.set_runtime_settings(None, None, None);
    session.set_codex_future_settings(json!({}));
    session.install_codex_startup();
    assert_eq!(session.next_turn_runtime_settings(), (None, None, None));
    assert_eq!(session.codex_connection.initial_choices, None);
    session.prepare_codex_startup(true, Some("none".into()));
    session.install_codex_startup();
    assert_eq!(
        session.codex_connection.initial_choices,
        Some(json!({"summary":"none"}))
    );
    assert_eq!(session.runtime_settings().current_summary(), &None);
}

#[test]
fn initial_marker_is_consumed_once_and_ordinary_reconnect_does_not_reapply_defaults() {
    let mut session = session(false, choices("saved", "max", "detailed"), None);
    session.install_codex_startup();
    session.codex_connection.initial_choices = None;
    session.queue_runtime_settings(None, None, None);
    session.set_runtime_settings(Some("new-native".into()), Some("low".into()), None);
    session.install_codex_startup();
    assert_eq!(session.codex_connection.initial_choices, None);
    assert_eq!(
        session.next_turn_runtime_settings(),
        (Some("new-native".into()), Some("low".into()), None)
    );
}

#[test]
fn empty_initial_catalog_uses_the_same_legacy_synthetic_models() {
    let mut session = session(false, RuntimeDefaults::default(), None);
    session.set_available_models(Vec::new());
    session.install_codex_startup();
    assert_eq!(
        session.codex_connection.initial_choices,
        Some(json!({"model":"gpt-5.6-sol","effort":"high"}))
    );
    assert_eq!(session.runtime_settings().available_models().len(), 3);
    assert_eq!(
        session.runtime_settings().current_model().as_deref(),
        Some("native")
    );
}

fn session(resumed: bool, defaults: RuntimeDefaults, summary: Option<String>) -> SessionState {
    let mut session = SessionState::new("root".into(), defaults, false);
    session.enable_codex();
    session.set_runtime_capabilities(true, true);
    session.set_runtime_settings(
        Some("native".into()),
        Some("low".into()),
        Some("native-summary".into()),
    );
    session.set_available_models(vec![
        model("native", &["low", "high"], true),
        model("saved", &["low", "max"], false),
    ]);
    session.set_codex_future_settings(session.codex_runtime_choices());
    session.prepare_codex_startup(resumed, summary);
    session
}

fn choices(model: &str, effort: &str, summary: &str) -> RuntimeDefaults {
    RuntimeDefaults {
        model: Some(model.into()),
        effort: Some(effort.into()),
        summary: Some(summary.into()),
    }
}

fn model(name: &str, efforts: &[&str], is_default: bool) -> ModelInfo {
    ModelInfo {
        model: name.into(),
        display_name: name.into(),
        supported_efforts: efforts.iter().map(|value| (*value).into()).collect(),
        default_effort: Some("low".into()),
        is_default,
    }
}
