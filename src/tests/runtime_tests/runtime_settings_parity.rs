// Enumerate settings rules that must survive moving backend choices into the session.

fn settings_values(values: [Option<&str>; 3]) -> (Option<String>, Option<String>, Option<String>) {
    (
        values[0].map(str::to_string),
        values[1].map(str::to_string),
        values[2].map(str::to_string),
    )
}

fn seed_runtime_choices() -> AppState {
    let mut app = AppState::new("settings".into());
    app.set_runtime_settings(
        Some("old".into()),
        Some("high".into()),
        Some("concise".into()),
    );
    app
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
            app.take_pending_runtime_settings(),
            settings_values([expected; 3])
        );
        assert_eq!(
            app.take_pending_runtime_settings(),
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
            app.take_pending_runtime_settings(),
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
        assert_eq!(app.take_pending_runtime_settings(), (None, None, None));
    }
    assert_eq!(
        AppState::new("empty".into()).runtime_settings_label(),
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
        let mut app = AppState::new("summary".into());
        app.set_runtime_settings(None, None, current.map(str::to_string));
        app.queue_runtime_settings(None, None, pending.map(str::to_string));
        app.apply_default_reasoning_summary(requested.map(str::to_string));
        assert_eq!(app.take_pending_runtime_settings().2.as_deref(), expected);
    }
}

#[test]
fn settings_editor_uses_pending_choices_and_stable_default_first_model_order() {
    let mut app = seed_runtime_choices();
    app.set_available_models(vec![
        runtime_test_model("other", &["low", "high"], "high", false),
        runtime_test_model("first", &["low", "high"], "low", true),
        runtime_test_model("second", &["low", "high"], "high", true),
        runtime_test_model("last", &["low", "high"], "low", false),
    ]);
    app.open_model_settings();
    assert_eq!(app.model_settings_model_value(), "first");
    for expected in ["second", "other", "last", "first"] {
        app.model_settings_cycle_model(1);
        assert_eq!(app.model_settings_model_value(), expected);
    }
    app.queue_runtime_settings(
        Some("second".into()),
        Some("LOW".into()),
        Some("none".into()),
    );
    app.open_model_settings();
    assert_eq!(app.model_settings_model_value(), "second");
    assert_eq!(app.model_settings_effort_value(), "low");
    assert_eq!(app.model_settings_summary_value(), "none");
    let defaults = app.apply_model_settings();
    assert_eq!(
        defaults,
        RuntimeDefaults {
            model: Some("second".into()),
            effort: Some("low".into()),
            summary: Some("none".into())
        }
    );
}

#[test]
fn settings_editor_effort_fallbacks_follow_requested_default_then_bounded_medium() {
    for (options, default, requested, expected) in [
        (vec!["low", "HIGH"], "low", "high", "HIGH"),
        (vec!["low", "high"], "high", "unknown", "high"),
        (vec!["low", "high"], "missing", "unknown", "high"),
        (vec![], "missing", "unknown", "medium"),
        (vec!["only"], "missing", "unknown", "only"),
    ] {
        let mut app = AppState::new("effort".into());
        app.set_available_models(vec![runtime_test_model("model", &options, default, true)]);
        app.queue_runtime_settings(
            Some("model".into()),
            Some(requested.into()),
            Some("unknown".into()),
        );
        app.open_model_settings();
        assert_eq!(app.model_settings_effort_value(), expected);
        assert_eq!(app.model_settings_summary_value(), "auto");
    }
}

#[test]
fn settings_editor_edits_and_cancel_leave_session_choices_untouched() {
    let mut app = seed_runtime_choices();
    let before = app.next_turn_runtime_settings();
    app.open_model_settings();
    assert_eq!(app.model_settings_model_value(), "old");
    app.model_settings_insert_char('é');
    assert_eq!(app.model_settings_model_value(), "oldé");
    app.model_settings_backspace();
    app.model_settings_cycle_model(-1);
    assert_eq!(app.model_settings_model_value(), "old");
    app.model_settings_cycle_effort(-1);
    assert_eq!(app.model_settings_effort_value(), "medium");
    app.model_settings_cycle_summary(-1);
    assert_eq!(app.model_settings_summary_value(), "auto");
    app.model_settings_move_field(false);
    app.close_model_settings();
    assert_eq!(app.next_turn_runtime_settings(), before);
    assert_eq!(app.take_pending_runtime_settings(), (None, None, None));
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
        crate::app::codex_settings::observed(
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
        crate::app::codex_settings_draft::reject(&mut app, &desired, json!({"message":"rejected"}));
        assert_eq!(
            app.take_pending_runtime_settings(),
            settings_values(expected)
        );
    }
}

#[test]
fn settings_launch_override_keeps_empty_and_spaced_native_strings() {
    for (requested, expected) in [
        (None, None),
        (Some(""), Some("")),
        (Some(" spaced "), Some(" spaced ")),
    ] {
        let mut app = seed_runtime_choices();
        let response = json!({"result":{"thread":{"id":"settings"},"model":"old","reasoningEffort":"high","reasoningSummary":"concise"}}).to_string();
        crate::app::codex_settings_draft::initialize(
            &mut app,
            &response,
            true,
            requested.map(str::to_string),
        )
        .expect("native settings");
        assert_eq!(app.take_pending_runtime_settings().2.as_deref(), expected);
    }
}

#[test]
fn settings_constructor_receives_startup_choices_without_loading_files() {
    for (input, expected) in [
        (None, None),
        (Some("  "), None),
        (Some(" chosen "), Some("chosen")),
    ] {
        let app = AppState::with_runtime_defaults(
            "startup".into(),
            RuntimeDefaults {
                model: input.map(str::to_string),
                effort: input.map(str::to_string),
                summary: input.map(str::to_string),
            },
        );
        assert_eq!(
            app.next_turn_runtime_settings(),
            settings_values([expected; 3])
        );
    }
}
