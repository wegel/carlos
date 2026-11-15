//! Keep shared settings separate from transcript text and each client's editor.

use carlos_proto::session::Backend;
use carlos_proto::settings::ModelInfo;

use super::events::Change;
use super::models::Role;
use super::state::SessionState;

#[test]
fn settings_publish_unknown_current_and_submitted_choices_separately() {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    let initial = session.publish().snapshot(10).settings.expect("settings");
    assert_eq!(initial.current.model, None);
    assert_eq!(initial.current.summary, None);
    session.set_runtime_settings(Some("native-model".into()), None, None);
    session.queue_runtime_settings(Some("next-model".into()), Some("high".into()), None);
    let batch = session.publish();
    assert!(batch
        .records()
        .iter()
        .any(|record| record.change == Change::Settings));
    let facts = batch.snapshot(10).settings.expect("changed settings");
    assert_eq!(facts.current.model.as_deref(), Some("native-model"));
    assert_eq!(facts.current.effort, None);
    assert_eq!(facts.pending.model.as_deref(), Some("next-model"));
    assert_eq!(facts.pending.effort.as_deref(), Some("high"));
    assert_ne!(facts.revision, initial.revision);
}

#[test]
fn transcript_changes_do_not_repeat_or_invalidate_settings() {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    let expected = session.publish().snapshot(10).settings;
    session.append_message(Role::Assistant, "ordinary answer");
    let batch = session.publish();
    assert!(batch
        .records()
        .iter()
        .all(|record| record.change != Change::Settings));
    assert_eq!(batch.snapshot(10).settings, expected);
}

#[test]
fn catalog_reordering_efforts_and_native_scope_fence_settings_reads() {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    let before = session.settings_revision();
    let mut models = vec![ModelInfo {
        model: "model".into(),
        display_name: "Model".into(),
        supported_efforts: vec!["low".into()],
        default_effort: None,
        is_default: true,
    }];
    session.set_available_models(models.clone());
    let first = session.settings_revision();
    assert_ne!(before, first);
    models[0].supported_efforts.push("high".into());
    session.set_available_models(models);
    let second = session.settings_revision();
    assert_ne!(first, second);
    session.configure_codex_connection(2, None);
    assert_ne!(second, session.settings_revision());
    let batch = session.publish();
    assert_eq!(
        batch.snapshot(10).settings.expect("settings").model_count,
        1
    );
}

#[test]
fn backend_capabilities_and_summary_preferences_are_numbered_facts() {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.set_client_identity(Backend::Claude, "host".into());
    session.set_runtime_capabilities(true, false);
    let before = session.publish().snapshot(10).settings.expect("settings");
    assert!(!before.supports_summary);
    session.set_runtime_capabilities(false, false);
    let batch = session.publish();
    assert!(batch
        .records()
        .iter()
        .any(|record| record.change == Change::Settings));
    assert!(
        !batch
            .snapshot(10)
            .settings
            .expect("settings")
            .supports_effort
    );
}
