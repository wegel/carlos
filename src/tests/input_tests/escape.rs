//! Escape bursts cannot turn a stop request into an edit of an earlier prompt.

use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use super::{handle_terminal_event, AppState, ApprovalRespondMock, TerminalSize};

#[test]
fn interrupt_discards_an_armed_chord_and_waits_for_a_quiet_interval() {
    let mut app = AppState::new("thread-1".to_string());
    let now = Instant::now();
    assert!(!app.register_escape_press(now));
    app.suppress_esc_chord_after_interrupt(now);

    for millis in [100, 600, 1100] {
        assert!(!app.register_escape_press(now + Duration::from_millis(millis)));
    }
    assert!(!app.register_escape_press(now + Duration::from_millis(1900)));
    assert!(app.register_escape_press(now + Duration::from_millis(2000)));
}

#[test]
fn another_key_lets_the_user_start_a_new_escape_chord() {
    let mut app = AppState::new("thread-1".to_string());
    let now = Instant::now();
    app.suppress_esc_chord_after_interrupt(now);

    app.input_insert_text("draft".to_string());

    assert!(!app.register_escape_press(now + Duration::from_millis(100)));
    assert!(app.register_escape_press(now + Duration::from_millis(200)));
}

#[test]
fn holding_escape_cannot_finish_an_idle_chord() {
    let client = ApprovalRespondMock::new();
    let mut app = AppState::new("thread-1".to_string());
    app.push_input_history("earlier prompt");
    assert!(!app.register_escape_press(Instant::now()));
    let repeat = KeyEvent::new_with_kind(KeyCode::Esc, KeyModifiers::NONE, KeyEventKind::Repeat);

    handle_terminal_event(
        &client,
        &mut app,
        Event::Key(repeat),
        TerminalSize {
            width: 100,
            height: 32,
        },
    );

    assert!(!app.rewind_mode());
    assert!(app.input_is_empty());
}
