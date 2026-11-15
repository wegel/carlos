//! Preserve exact empty, pending, capability and stale native settings rules in core.

use serde_json::json;

use crate::session::state::SessionState;

fn settings_values(values: [Option<&str>; 3]) -> (Option<String>, Option<String>, Option<String>) {
    (
        values[0].map(str::to_string),
        values[1].map(str::to_string),
        values[2].map(str::to_string),
    )
}

fn seed_runtime_choices() -> SessionState {
    let mut session = SessionState::new("settings".into(), Default::default(), false);
    session.set_runtime_settings(
        Some("old".into()),
        Some("high".into()),
        Some("concise".into()),
    );
    session
}

#[test]
fn settings_set_merge_and_queue_keep_their_distinct_empty_value_rules() {
    for (input, expected) in [
        (None, None),
        (Some(" \n "), None),
        (Some(" new \t"), Some("new")),
    ] {
        let mut app = seed_runtime_choices();
        let (model, effort, summary) = settings_values([input; 3]);
        app.merge_runtime_settings(model, effort, summary);
        let merged = expected.map(|value| [Some(value); 3]).unwrap_or([
            Some("old"),
            Some("high"),
            Some("concise"),
        ]);
        assert_eq!(app.next_turn_runtime_settings(), settings_values(merged));
        let (model, effort, summary) = settings_values([input; 3]);
        app.set_runtime_settings(model, effort, summary);
        assert_eq!(
            app.next_turn_runtime_settings(),
            settings_values([expected; 3])
        );
        let (model, effort, summary) = settings_values([input; 3]);
        app.queue_runtime_settings(model, effort, summary);
        assert_eq!(
            app.pending_runtime_settings(),
            settings_values([expected; 3])
        );
        assert_eq!(
            app.pending_runtime_settings(),
            settings_values([expected; 3])
        );
    }
}

#[test]
fn settings_capability_changes_clear_values_but_only_merge_checks_capabilities() {
    for (effort, summary, expected) in [
        (true, true, [Some("queued"), Some("low"), Some("none")]),
        (true, false, [Some("queued"), Some("low"), None]),
        (false, true, [Some("queued"), None, Some("none")]),
        (false, false, [Some("queued"), None, None]),
    ] {
        let mut app = seed_runtime_choices();
        app.queue_runtime_settings(
            Some("queued".into()),
            Some("low".into()),
            Some("none".into()),
        );
        app.set_runtime_capabilities(effort, summary);
        assert_eq!(app.next_turn_runtime_settings(), settings_values(expected));
        app.queue_runtime_settings(None, None, None);
        app.merge_runtime_settings(
            Some("merged".into()),
            Some("medium".into()),
            Some("detailed".into()),
        );
        let expected = [
            Some("merged"),
            effort.then_some("medium"),
            summary.then_some("detailed"),
        ];
        assert_eq!(app.next_turn_runtime_settings(), settings_values(expected));
        app.set_runtime_settings(Some("set".into()), Some("high".into()), Some("auto".into()));
        assert_eq!(
            app.next_turn_runtime_settings(),
            settings_values([Some("set"), Some("high"), Some("auto")])
        );
        app.queue_runtime_settings(Some("q".into()), Some("low".into()), Some("none".into()));
        assert_eq!(
            app.pending_runtime_settings(),
            settings_values([Some("q"), Some("low"), Some("none")])
        );
    }
}

#[test]
fn settings_partial_pending_choices_replace_all_current_fields_when_applied() {
    for (pending, next, applied, label) in [
        (
            [None, None, None],
            [Some("old"), Some("high"), Some("concise")],
            [Some("old"), Some("high"), Some("concise")],
            "old/high/concise",
        ),
        (
            [Some("new"), None, None],
            [Some("new"), Some("high"), Some("concise")],
            [Some("new"), None, None],
            "new/high/concise*",
        ),
        (
            [None, Some("low"), None],
            [Some("old"), Some("low"), Some("concise")],
            [None, Some("low"), None],
            "old/low/concise*",
        ),
        (
            [None, None, Some("none")],
            [Some("old"), Some("high"), Some("none")],
            [None, None, Some("none")],
            "old/high/none*",
        ),
        (
            [Some("old"), Some("high"), Some("concise")],
            [Some("old"), Some("high"), Some("concise")],
            [Some("old"), Some("high"), Some("concise")],
            "old/high/concise",
        ),
        (
            [Some("old"), None, None],
            [Some("old"), Some("high"), Some("concise")],
            [Some("old"), None, None],
            "old/high/concise*",
        ),
    ] {
        let mut app = seed_runtime_choices();
        let (model, effort, summary) = settings_values(pending);
        app.queue_runtime_settings(model, effort, summary);
        assert_eq!(app.next_turn_runtime_settings(), settings_values(next));
        assert_eq!(app.runtime_settings_label(), label);
        app.mark_runtime_settings_applied();
        assert_eq!(app.next_turn_runtime_settings(), settings_values(applied));
        assert_eq!(app.pending_runtime_settings(), (None, None, None));
    }
    assert_eq!(
        SessionState::new("empty".into(), Default::default(), false).runtime_settings_label(),
        "model?"
    );
}

#[test]
fn settings_default_summary_never_overwrites_current_or_pending_summary() {
    for (current, pending, requested, expected) in [
        (None, None, Some(" detailed "), Some("detailed")),
        (None, None, Some(" \t"), None),
        (None, None, None, None),
        (Some("concise"), None, Some("detailed"), None),
        (None, Some("none"), Some("detailed"), Some("none")),
        (
            Some("concise"),
            Some("none"),
            Some("detailed"),
            Some("none"),
        ),
    ] {
        let mut app = SessionState::new("summary".into(), Default::default(), false);
        app.set_runtime_settings(None, None, current.map(str::to_string));
        app.queue_runtime_settings(None, None, pending.map(str::to_string));
        app.apply_default_reasoning_summary(requested.map(str::to_string));
        assert_eq!(app.pending_runtime_settings().2.as_deref(), expected);
    }
}

#[test]
fn settings_native_replies_keep_absent_null_empty_and_stale_choices_distinct() {
    for (settings, expected) in [
        (json!({}), [Some("old"), Some("high"), Some("concise")]),
        (json!({"model":null}), [None, Some("high"), Some("concise")]),
        (json!({"effort":17}), [Some("old"), None, Some("concise")]),
        (json!({"summary":""}), [Some("old"), Some("high"), Some("")]),
        (
            json!({"model":" spaced "}),
            [Some(" spaced "), Some("high"), Some("concise")],
        ),
    ] {
        let mut app = seed_runtime_choices();
        crate::session::codex::settings::observed(
            &mut app,
            &json!({"threadId":"settings", "threadSettings":settings}),
        );
        assert_eq!(app.next_turn_runtime_settings(), settings_values(expected));
    }
    for (desired, expected) in [
        (json!({"model":"new"}), [None, Some("low"), Some("none")]),
        (
            json!({"model":"stale", "effort":"low"}),
            [Some("new"), None, Some("none")],
        ),
        (
            json!({"model":null, "summary":"none"}),
            [Some("new"), Some("low"), None],
        ),
    ] {
        let mut app = seed_runtime_choices();
        app.queue_runtime_settings(Some("new".into()), Some("low".into()), Some("none".into()));
        let mut update = crate::session::codex::settings::SettingsUpdate::default();
        crate::session::codex::settings::reject(
            &mut app,
            &mut update,
            &desired,
            json!({"message":"rejected"}),
        );
        assert_eq!(app.pending_runtime_settings(), settings_values(expected));
    }
}

#[test]
fn model_catalog_keeps_defaults_first_and_preserves_equal_priority_order() {
    let mut session = seed_runtime_choices();
    session.set_available_models(
        [
            ("other", false),
            ("first", true),
            ("second", true),
            ("last", false),
        ]
        .into_iter()
        .map(|(name, is_default)| carlos_proto::settings::ModelInfo {
            model: name.into(),
            display_name: name.into(),
            supported_efforts: vec!["high".into()],
            default_effort: Some("high".into()),
            is_default,
        })
        .collect(),
    );
    let names: Vec<_> = session
        .runtime_settings()
        .available_models()
        .iter()
        .map(|model| model.model.as_str())
        .collect();
    assert_eq!(names, ["first", "second", "other", "last"]);
}
