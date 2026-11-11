//! Preserve resumed thread choices while allowing explicit startup and editor overrides.

use serde_json::json;

use super::codex_question_support::{app, Peer};
use crate::app::codex_settings::tick;
use crate::app::input_events::handle_terminal_event;
use crate::app::TerminalSize;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

#[test]
fn resumed_native_settings_are_not_republished_and_unknown_summary_stays_inherited() {
    let mut app = app();
    let peer = Peer::default();
    app.set_runtime_settings(
        Some("carlos-default".into()),
        Some("high".into()),
        Some("concise".into()),
    );
    crate::app::codex_settings_draft::initialize(
        &mut app,
        r#"{"result":{"model":"native","reasoningEffort":"low","thread":{"id":"root"}}}"#,
        true,
        None,
    )
    .expect("native resume");
    assert_eq!(
        app.next_turn_runtime_settings(),
        (Some("native".into()), Some("low".into()), None)
    );
    assert!(!tick(&peer, &mut app));
    assert!(peer.controls.borrow().is_empty());
    for _ in 0..2 {
        app.open_model_settings();
        assert_eq!(app.model_settings_summary_value(), "Use thread setting");
        app.close_model_settings();
    }
}

#[test]
fn changing_model_in_the_editor_does_not_publish_an_unknown_summary() {
    let mut app = app();
    let peer = Peer::default();
    app.set_runtime_capabilities(true, true);
    app.set_runtime_settings(Some("native".into()), Some("low".into()), None);
    app.open_model_settings();
    app.runtime.model_settings_model_input = "edited".into();
    handle_terminal_event(
        &peer,
        &mut app,
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        TerminalSize {
            width: 100,
            height: 32,
        },
    );
    assert_eq!(peer.controls.borrow()[0].1["model"], "edited");
    assert!(peer.controls.borrow()[0].1.get("summary").is_none());
    assert!(app.runtime.pending_summary.is_none());
    assert!(app.runtime.current_summary.is_none());
}

#[test]
fn explicit_launch_summary_is_published_on_resume_and_new_thread_defaults_remain_intentional() {
    let mut app = app();
    let peer = Peer::default();
    let response =
        r#"{"result":{"model":"native","reasoningEffort":"low","thread":{"id":"root"}}}"#;
    crate::app::codex_settings_draft::initialize(&mut app, response, true, Some("detailed".into()))
        .expect("explicit resume");
    assert!(tick(&peer, &mut app));
    assert_eq!(
        peer.controls.borrow()[0].1,
        json!({"threadId":"root","summary":"detailed"})
    );
    app.codex.settings.pending = None;
    app.queue_runtime_settings(None, None, None);
    app.set_runtime_settings(
        Some("native".into()),
        Some("low".into()),
        Some("concise".into()),
    );
    crate::app::codex_settings_draft::initialize(&mut app, response, false, None)
        .expect("new thread defaults");
    assert!(tick(&peer, &mut app));
    assert_eq!(
        peer.controls.borrow()[1].1,
        json!({"threadId":"root","summary":"concise"})
    );
}

#[test]
fn explicitly_choosing_a_summary_in_the_editor_still_publishes_it() {
    let mut app = app();
    let peer = Peer::default();
    app.set_runtime_capabilities(true, true);
    app.set_runtime_settings(Some("native".into()), Some("low".into()), None);
    app.open_model_settings();
    app.model_settings_cycle_summary(1);
    assert_eq!(app.model_settings_summary_value(), "auto");
    handle_terminal_event(
        &peer,
        &mut app,
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        TerminalSize {
            width: 100,
            height: 32,
        },
    );
    assert_eq!(peer.controls.borrow()[0].1["summary"], "auto");
}
