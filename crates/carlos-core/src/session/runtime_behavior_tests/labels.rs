//! Preserve compact labels and pending next-turn choices in the session.

use crate::session::state::SessionState;

#[test]
fn runtime_settings_label_shows_pending_summary_override() {
    let mut app = SessionState::new("thread-1".into(), Default::default(), false);
    app.set_runtime_settings(Some("gpt-5".to_string()), Some("high".to_string()), None);
    app.apply_default_reasoning_summary(Some("auto".to_string()));

    assert_eq!(app.runtime_settings_label(), "gpt-5/high/auto*");
}

#[test]
fn runtime_settings_label_omits_missing_trailing_fields() {
    let mut app = SessionState::new("thread-1".into(), Default::default(), false);
    app.set_runtime_settings(Some("claude-opus-4-6".to_string()), None, None);

    assert_eq!(app.runtime_settings_label(), "claude-opus-4-6");
}

#[test]
fn next_turn_runtime_settings_falls_back_to_current_values() {
    let mut app = SessionState::new("thread-1".into(), Default::default(), false);
    app.set_runtime_settings(
        Some("gpt-5.4".to_string()),
        Some("high".to_string()),
        Some("concise".to_string()),
    );

    assert_eq!(
        app.next_turn_runtime_settings(),
        (
            Some("gpt-5.4".to_string()),
            Some("high".to_string()),
            Some("concise".to_string())
        )
    );
}

#[test]
fn next_turn_runtime_settings_prefers_pending_over_current_values() {
    let mut app = SessionState::new("thread-1".into(), Default::default(), false);
    app.set_runtime_settings(
        Some("gpt-5.4".to_string()),
        Some("high".to_string()),
        Some("concise".to_string()),
    );
    app.queue_runtime_settings(
        Some("gpt-5.4-mini".to_string()),
        Some("medium".to_string()),
        Some("auto".to_string()),
    );

    assert_eq!(
        app.next_turn_runtime_settings(),
        (
            Some("gpt-5.4-mini".to_string()),
            Some("medium".to_string()),
            Some("auto".to_string())
        )
    );
}
