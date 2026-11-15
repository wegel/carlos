//! Exercise Escape repeats and pasted text through the actual device without a backend owner.

use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::app::remote::widget_fixture::device;
use crate::app::view_state::ViewState;

#[test]
fn interrupt_discards_an_armed_chord_and_waits_for_a_quiet_interval() {
    let mut view = ViewState::new(false);
    let now = Instant::now();
    assert!(!view.register_escape_press(now));
    view.suppress_esc_chord_after_interrupt(now);
    for millis in [100, 600, 1100] {
        assert!(!view.register_escape_press(now + Duration::from_millis(millis)));
    }
    assert!(!view.register_escape_press(now + Duration::from_millis(1900)));
    assert!(view.register_escape_press(now + Duration::from_millis(2000)));
}

#[test]
fn pasted_composer_text_lets_the_user_start_a_new_escape_chord() {
    let mut device = device(Vec::new(), false);
    let now = Instant::now();
    device.view.suppress_esc_chord_after_interrupt(now);
    device
        .event(None, Event::Paste("draft".into()))
        .expect("paste");
    assert_eq!(device.view.input.lines().join("\n"), "draft");
    assert!(!device
        .view
        .register_escape_press(now + Duration::from_millis(100)));
    assert!(device
        .view
        .register_escape_press(now + Duration::from_millis(200)));
}

#[test]
fn holding_escape_cannot_finish_an_idle_chord() {
    let mut device = device(Vec::new(), false);
    let now = Instant::now();
    assert!(!device.view.register_escape_press(now));
    let repeat = KeyEvent::new_with_kind(KeyCode::Esc, KeyModifiers::NONE, KeyEventKind::Repeat);
    device.event(None, Event::Key(repeat)).expect("repeat key");
    assert_eq!(device.view.esc_armed_at, Some(now));
    assert!(device.view.status_hint.is_none());
    assert!(device.history.rewind_draft.is_none());
    assert!(device.view.input.is_empty());
    assert!(device.client.operations().iter().next().is_none());
}
