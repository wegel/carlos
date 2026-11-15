//! Check borrowed settings editors without retaining or mutating a core session.

use carlos_core::session::runtime_settings::RuntimeDefaults;
use carlos_proto::settings::{ModelInfo, RuntimeChoices, SettingsFacts};

use super::{RuntimeSettingsRef, RuntimeSettingsView, DEFAULT_SUMMARY_OPTIONS};

fn facts() -> SettingsFacts {
    SettingsFacts {
        revision: "b".repeat(64).parse().expect("settings revision"),
        current: RuntimeChoices {
            model: Some("old".into()),
            effort: Some("high".into()),
            summary: Some("concise".into()),
        },
        pending: RuntimeChoices::default(),
        supports_effort: true,
        supports_summary: true,
        model_count: 0,
        native_pending: false,
        legacy_overrides: false,
        label: "old/high/concise".into(),
        summaries_enabled: false,
    }
}

fn model(name: &str, efforts: &[&str], default: &str, is_default: bool) -> ModelInfo {
    ModelInfo {
        model: name.into(),
        display_name: name.into(),
        supported_efforts: efforts.iter().map(|value| (*value).into()).collect(),
        default_effort: Some(default.into()),
        is_default,
    }
}

#[test]
fn editor_cycles_the_advertised_order_and_prefers_pending_choices() {
    let mut facts = facts();
    let models = vec![
        model("first", &["low", "high"], "low", true),
        model("second", &["low", "high"], "high", true),
        model("other", &["low", "high"], "high", false),
        model("last", &["low", "high"], "low", false),
    ];
    let mut editor = RuntimeSettingsView::new();
    editor.open_model_settings(RuntimeSettingsRef::from((&facts, models.as_slice())));
    assert_eq!(
        editor.model_settings_model_value((&facts, models.as_slice()).into()),
        "first"
    );
    for expected in ["second", "other", "last", "first"] {
        editor.model_settings_cycle_model((&facts, models.as_slice()).into(), 1);
        assert_eq!(
            editor.model_settings_model_value((&facts, models.as_slice()).into()),
            expected
        );
    }
    facts.pending = RuntimeChoices {
        model: Some("second".into()),
        effort: Some("LOW".into()),
        summary: Some("none".into()),
    };
    editor.open_model_settings((&facts, models.as_slice()).into());
    assert_eq!(
        editor.model_settings_model_value((&facts, models.as_slice()).into()),
        "second"
    );
    assert_eq!(editor.model_settings_effort_value(), "low");
    assert_eq!(
        editor.model_settings_summary_value((&facts, models.as_slice()).into()),
        "none"
    );
    assert_eq!(
        editor.saved_choices((&facts, models.as_slice()).into()),
        RuntimeDefaults {
            model: Some("second".into()),
            effort: Some("low".into()),
            summary: Some("none".into())
        }
    );
}

#[test]
fn effort_fallbacks_follow_requested_default_then_bounded_medium() {
    for (options, default, requested, expected) in [
        (vec!["low", "HIGH"], "low", "high", "HIGH"),
        (vec!["low", "high"], "high", "unknown", "high"),
        (vec!["low", "high"], "missing", "unknown", "high"),
        (vec![], "missing", "unknown", "medium"),
        (vec!["only"], "missing", "unknown", "only"),
    ] {
        let mut facts = facts();
        facts.pending = RuntimeChoices {
            model: Some("model".into()),
            effort: Some(requested.into()),
            summary: Some("unknown".into()),
        };
        let models = [model("model", &options, default, true)];
        let mut editor = RuntimeSettingsView::new();
        editor.open_model_settings((&facts, models.as_slice()).into());
        assert_eq!(editor.model_settings_effort_value(), expected);
        assert_eq!(
            editor.model_settings_summary_value((&facts, models.as_slice()).into()),
            "auto"
        );
    }
}

#[test]
fn edits_and_cancel_leave_borrowed_facts_untouched() {
    let facts = facts();
    let before = facts.clone();
    let runtime = RuntimeSettingsRef::from((&facts, [].as_slice()));
    let mut editor = RuntimeSettingsView::new();
    editor.open_model_settings(runtime);
    assert_eq!(editor.model_settings_model_value(runtime), "old");
    editor.model_settings_insert_char('é');
    assert_eq!(editor.model_settings_model_value(runtime), "oldé");
    editor.model_settings_backspace();
    editor.model_settings_cycle_model(runtime, -1);
    assert_eq!(editor.model_settings_model_value(runtime), "old");
    editor.model_settings_cycle_effort(-1);
    assert_eq!(editor.model_settings_effort_value(), "medium");
    editor.model_settings_cycle_summary(-1);
    assert_eq!(editor.model_settings_summary_value(runtime), "auto");
    editor.model_settings_move_field(runtime, false);
    editor.close_model_settings();
    assert_eq!(facts, before);
    assert!(!editor.show_model_settings);
}

#[test]
fn saved_choices_keep_exact_model_effort_and_supported_summary() {
    for supports_summary in [true, false] {
        let mut facts = facts();
        facts.supports_summary = supports_summary;
        let runtime = RuntimeSettingsRef::from((&facts, [].as_slice()));
        let mut editor = RuntimeSettingsView::new();
        editor.model_settings_model_input = "gpt-5.4".into();
        editor.model_settings_effort_index = 4;
        editor.model_settings_summary_options = DEFAULT_SUMMARY_OPTIONS
            .iter()
            .map(|value| (*value).into())
            .collect();
        editor.model_settings_summary_index = 1;
        assert_eq!(
            editor.saved_choices(runtime),
            RuntimeDefaults {
                model: Some("gpt-5.4".into()),
                effort: Some("high".into()),
                summary: supports_summary.then(|| "concise".into())
            }
        );
        assert_eq!(facts.pending, RuntimeChoices::default());
    }
}
