//! Keep terminal model drafts private while unknown native fields remain inherited.

use carlos_proto::ids::Sequence;
use carlos_proto::session::{Backend, Snapshot};
use carlos_proto::settings::{ModelInfo, RuntimeChoices, SettingValue, SettingsFacts};

use super::Catalog;
use crate::app::remote::settings_changes;
use crate::app::remote::widget_fixture::{device_with_snapshot, info};
use crate::app::remote::Device;
use crate::app::runtime_settings_state::ModelSettingsField;

fn catalog(models: Vec<ModelInfo>) -> Catalog {
    Catalog {
        facts: SettingsFacts {
            revision: "e".repeat(64).parse().expect("settings revision"),
            current: RuntimeChoices {
                model: Some("native".into()),
                effort: Some("low".into()),
                summary: None,
            },
            pending: RuntimeChoices::default(),
            supports_effort: true,
            supports_summary: true,
            model_count: models.len() as u32,
            native_pending: false,
            legacy_overrides: false,
            label: "native/low".into(),
            summaries_enabled: false,
        },
        models,
    }
}

fn device(catalog: Catalog) -> Device {
    let session = "a".repeat(32).parse().expect("logical session");
    let snapshot = Snapshot {
        info: info(session, false),
        entries: Vec::new(),
        approvals: Vec::new(),
        seq: Sequence::new(1),
        has_older: false,
        goal: None,
        queue: None,
        settings: Some(catalog.facts.clone()),
        tasks: None,
    };
    let (mut device, _) = device_with_snapshot(snapshot);
    device.install_settings_catalog(catalog);
    device
}

fn change(device: &Device) -> carlos_proto::settings::SettingsChange {
    settings_changes::change(
        &device.view.runtime_settings,
        device.settings.catalog.as_ref().expect("accepted catalog"),
        device.settings.initial.as_ref().expect("editor choices"),
        Backend::Codex,
    )
    .expect("valid private choices")
}

#[test]
fn unknown_summary_stays_inherited_across_two_editor_openings() {
    let mut device = device(catalog(Vec::new()));
    for _ in 0..2 {
        let accepted = device.settings.catalog.as_ref().expect("catalog");
        assert_eq!(
            device
                .view
                .runtime_settings
                .model_settings_summary_value(accepted.runtime()),
            "Use thread setting"
        );
        assert!(change(&device).summary.is_none());
        device.close_settings();
        device.install_settings_catalog(catalog(Vec::new()));
    }
    assert!(device.client.operations().iter().next().is_none());
    assert!(device
        .client
        .session(&device.session)
        .expect("accepted facts")
        .settings()
        .expect("settings")
        .current
        .summary
        .is_none());
}

#[test]
fn changing_only_the_model_does_not_publish_an_unknown_summary() {
    let mut device = device(catalog(Vec::new()));
    device.view.runtime_settings.model_settings_model_input = "edited".into();
    assert_eq!(
        change(&device).model,
        Some(SettingValue::Value("edited".into()))
    );
    assert_eq!(change(&device).summary, None);
    let facts = device
        .client
        .session(&device.session)
        .expect("replica")
        .settings()
        .expect("settings");
    assert_eq!(
        (&facts.current.summary, &facts.pending.summary),
        (&None, &None)
    );
    assert!(device.client.operations().iter().next().is_none());
}

#[test]
fn explicitly_choosing_a_summary_publishes_only_that_private_choice() {
    let mut device = device(catalog(Vec::new()));
    device.view.runtime_settings.model_settings_cycle_summary(1);
    let catalog = device.settings.catalog.as_ref().expect("catalog");
    assert_eq!(
        device
            .view
            .runtime_settings
            .model_settings_summary_value(catalog.runtime()),
        "auto"
    );
    assert_eq!(
        change(&device).summary,
        Some(SettingValue::Value("auto".into()))
    );
    assert!(device
        .client
        .session(&device.session)
        .expect("replica")
        .settings()
        .expect("settings")
        .current
        .summary
        .is_none());
    assert!(device.client.operations().iter().next().is_none());
}

#[test]
fn a_native_model_without_effort_choices_hides_the_effort_field() {
    let mut catalog = catalog(vec![ModelInfo {
        model: "native".into(),
        display_name: "Native".into(),
        supported_efforts: Vec::new(),
        default_effort: None,
        is_default: true,
    }]);
    catalog.facts.current.effort = Some("old-effort".into());
    let device = device(catalog);
    let catalog = device.settings.catalog.as_ref().expect("catalog");
    assert!(!device
        .view
        .runtime_settings
        .model_settings_fields(catalog.runtime())
        .contains(&ModelSettingsField::Effort));
    assert_eq!(change(&device).effort, None);
    assert!(device.client.operations().iter().next().is_none());
}

#[test]
fn rejected_model_and_effort_survive_a_catalog_refresh_without_becoming_native_facts() {
    let mut device = device(catalog(Vec::new()));
    let attempted = carlos_proto::settings::SettingsChange {
        model: Some(SettingValue::Value("rejected".into())),
        effort: Some(SettingValue::Value("high".into())),
        summary: None,
    };
    let accepted = device.settings.catalog.as_ref().expect("catalog");
    settings_changes::restore(
        &mut device.view.runtime_settings,
        accepted,
        attempted.clone(),
    );
    device.settings_disconnected();
    device.install_settings_catalog(catalog(Vec::new()));
    let accepted = device.settings.catalog.as_ref().expect("fresh catalog");
    assert_eq!(
        device
            .view
            .runtime_settings
            .model_settings_model_value(accepted.runtime()),
        "rejected"
    );
    assert_eq!(
        device.view.runtime_settings.model_settings_effort_value(),
        "high"
    );
    assert_eq!(change(&device), attempted);
    let facts = device
        .client
        .session(&device.session)
        .expect("replica")
        .settings()
        .expect("settings");
    assert_eq!(facts.current.model.as_deref(), Some("native"));
    assert_eq!(facts.current.effort.as_deref(), Some("low"));
    assert_eq!(facts.pending, RuntimeChoices::default());
    assert!(device.client.operations().iter().next().is_none());
}

#[test]
fn an_invalid_effort_stays_in_the_private_editor_for_the_owner_to_reject() {
    let mut device = device(catalog(vec![ModelInfo {
        model: "native".into(),
        display_name: "Native".into(),
        supported_efforts: vec!["low".into()],
        default_effort: Some("low".into()),
        is_default: true,
    }]));
    device
        .view
        .runtime_settings
        .model_settings_effort_options
        .push("rejected-effort".into());
    device.view.runtime_settings.model_settings_effort_index = 1;
    assert!(device.view.runtime_settings.show_model_settings);
    assert_eq!(
        device.view.runtime_settings.model_settings_effort_value(),
        "rejected-effort"
    );
    assert_eq!(
        change(&device).effort,
        Some(SettingValue::Value("rejected-effort".into()))
    );
    assert_eq!(
        device
            .client
            .session(&device.session)
            .expect("replica")
            .settings()
            .expect("settings")
            .current
            .effort
            .as_deref(),
        Some("low")
    );
    assert!(device.client.operations().iter().next().is_none());
}

#[test]
fn catalog_reordering_preserves_the_private_model_by_value() {
    let model = |name: &str| ModelInfo {
        model: name.into(),
        display_name: name.to_uppercase(),
        supported_efforts: Vec::new(),
        default_effort: None,
        is_default: false,
    };
    let mut initial = catalog(vec![model("a"), model("b")]);
    initial.facts.current.model = Some("a".into());
    let mut device = device(initial);
    device.view.runtime_settings.model_settings_model_index = 1;
    device.settings_disconnected();
    let mut fresh = catalog(vec![model("b"), model("a")]);
    fresh.facts.current.model = Some("a".into());
    device.install_settings_catalog(fresh);
    let accepted = device.settings.catalog.as_ref().expect("new catalog");
    assert_eq!(
        device
            .view
            .runtime_settings
            .model_settings_model_value(accepted.runtime()),
        "b"
    );
    assert_eq!(accepted.facts.current.model.as_deref(), Some("a"));
    assert_eq!(accepted.facts.pending.model, None);
    assert!(device.client.operations().iter().next().is_none());
}
