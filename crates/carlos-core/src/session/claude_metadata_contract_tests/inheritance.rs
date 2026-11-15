//! Preserve Claude inherited choices at their actual Core APIs.

use serde_json::json;

use super::{app, choices};
use crate::session::claude::ClaudePhase;
use crate::session::runtime_settings::RuntimeDefaults;

#[test]
fn editor_saves_preserve_inheritance_only_for_unchanged_model_and_nonexplicit_effort() {
    for (editing, inherited, model, effort, expected) in [
        (
            false,
            true,
            "fast",
            "high",
            json!(["fast", "high", [null, null], "fast", "high"]),
        ),
        (
            true,
            true,
            "fast",
            "high",
            json!([null, null, [null, null], null, null]),
        ),
        (
            true,
            true,
            "small",
            "low",
            json!(["small", "low", ["chosen", "effort"], "small", "low"]),
        ),
        (
            true,
            false,
            "fast",
            "auto",
            json!(["fast", null, ["chosen", null], "fast", null]),
        ),
        (
            true,
            false,
            "fast",
            "high",
            json!(["fast", "high", ["chosen", "effort"], "fast", "high"]),
        ),
    ] {
        let mut app = app(Some("fast"), Some("high"));
        app.set_claude_inheritance_for_test(inherited, inherited);
        let initial = ("fast".into(), "high".into());
        app.queue_runtime_settings(Some(model.into()), Some(effort.into()), None);
        let mut defaults = saved_defaults(model, effort);
        app.prepare_saved_claude_choices(editing.then_some(&initial), &mut defaults);
        assert_eq!(
            json!([
                defaults.model,
                defaults.effort,
                app.claude_prompt_choices(Some("chosen".into()), Some("effort".into())),
                app.runtime_settings().pending_model(),
                app.runtime_settings().pending_effort()
            ]),
            expected
        );
        assert_eq!(defaults.summary.as_deref(), Some("keep"));
    }
}

#[test]
fn switching_clears_only_inherited_current_choices_without_pending_overrides() {
    for (inherited, pending, expected) in [
        (true, false, json!([null, null, null, null, [null, null]])),
        (
            true,
            true,
            json!(["fast", "high", "small", "low", [null, null]]),
        ),
        (
            false,
            false,
            json!(["fast", "high", null, null, ["chosen", "effort"]]),
        ),
        (
            false,
            true,
            json!(["fast", "high", "small", "low", ["chosen", "effort"]]),
        ),
    ] {
        let mut app = app(Some("fast"), Some("high"));
        app.set_claude_inheritance_for_test(inherited, inherited);
        if pending {
            app.queue_runtime_settings(Some("small".into()), Some("low".into()), None);
        }
        app.switch_claude_session(2, "branch", &mut Default::default());
        assert_eq!(choices(&app), expected);
        assert_eq!(app.claude().phase() as u8, ClaudePhase::Starting as u8);
        assert!(app.claude().capabilities().is_none());
    }
}

fn saved_defaults(model: &str, effort: &str) -> RuntimeDefaults {
    RuntimeDefaults {
        model: Some(model.into()),
        effort: Some(effort.into()),
        summary: Some("keep".into()),
    }
}

#[test]
fn launch_omits_only_missing_model_and_missing_or_auto_effort() {
    for (model, effort, expected) in [
        (None, None, json!([null, null])),
        (Some("fast"), None, json!(["m", null])),
        (None, Some("high"), json!([null, "e"])),
        (Some("fast"), Some("auto"), json!(["m", null])),
        (Some("fast"), Some("high"), json!(["m", "e"])),
    ] {
        let mut app = app(model, effort);
        app.start_claude_settings();
        assert_eq!(app.claude().phase() as u8, ClaudePhase::Starting as u8);
        assert_eq!(
            json!(app.claude_prompt_choices(Some("m".into()), Some("e".into()))),
            expected
        );
    }
}

#[test]
fn model_and_effort_inheritance_filter_prompt_choices_independently() {
    for (model, effort, expected) in [
        (true, true, json!([null, null])),
        (false, true, json!(["m", null])),
        (true, false, json!([null, "e"])),
        (false, false, json!(["m", "e"])),
    ] {
        let mut app = app(None, None);
        app.set_claude_inheritance_for_test(model, effort);
        assert_eq!(
            json!(app.claude_prompt_choices(Some("m".into()), Some("e".into()))),
            expected
        );
    }
}
