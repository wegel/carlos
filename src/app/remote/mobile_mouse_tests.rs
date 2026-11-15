//! Preserve broken mobile terminal mouse sequences without creating a native session owner.

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use crate::app::remote::widget_fixture::device;

#[test]
fn fragmented_alt_mouse_sequence_scrolls_without_entering_the_composer() {
    let mut device = device(Vec::new(), false);
    device.view.viewport.scroll_top = 12;
    device.view.viewport.mobile_mouse_last_y = Some(40);
    for ch in ['[', '<', '6', '4', ';', '7', '6', ';', '4', '6', 'M'] {
        device
            .event(
                None,
                Event::Key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::ALT)),
            )
            .expect("mobile mouse fragment");
    }
    assert_eq!(device.view.viewport.scroll_top, 18);
    assert!(device.view.viewport.mobile_mouse_buffer.is_empty());
    assert!(device.view.input.is_empty());
    assert!(device.client.operations().iter().next().is_none());
}

#[test]
fn ordinary_digits_and_failed_mouse_prefixes_keep_their_original_text() {
    let mut device = device(Vec::new(), false);
    for ch in "123;456".chars() {
        device
            .event(
                None,
                Event::Key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)),
            )
            .expect("ordinary digit");
    }
    assert_eq!(device.view.input.lines(), &["123;456"]);
    for ch in "<oops".chars() {
        device
            .event(
                None,
                Event::Key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)),
            )
            .expect("failed mouse prefix");
    }
    assert_eq!(device.view.input.lines(), &["123;456<oops"]);
    assert!(device.client.operations().iter().next().is_none());
}
