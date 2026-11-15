//! Preserve every existing mobile decoder rule on the actual local viewport.

use crossterm::event::{KeyCode, KeyModifiers};

use super::{
    apply_mobile_mouse_scroll, consume_mobile_mouse_char, is_mobile_mouse_key_candidate,
    parse_mobile_mouse_coords, parse_repeated_plain_mobile_pair, MobileMouseConsume,
};
use crate::app::viewport_state::ViewportState;

// Tests for mobile-mouse decoding and perf-overlay key handling.

#[test]
fn consume_mobile_mouse_char_requires_prefix_to_activate() {
    let mut viewport = ViewportState::new();
    assert!(matches!(
        consume_mobile_mouse_char(&mut viewport, '<'),
        MobileMouseConsume::Consumed
    ));
    assert!(matches!(
        consume_mobile_mouse_char(&mut viewport, '7'),
        MobileMouseConsume::Consumed
    ));
    assert!(matches!(
        consume_mobile_mouse_char(&mut viewport, '6'),
        MobileMouseConsume::Consumed
    ));
    assert!(matches!(
        consume_mobile_mouse_char(&mut viewport, ';'),
        MobileMouseConsume::Consumed
    ));
}

#[test]
fn consume_mobile_mouse_char_accepts_csi_bracket_prefix() {
    let mut viewport = ViewportState::new();
    viewport.scroll_top = 12;
    viewport.mobile_mouse_last_y = Some(40);

    for ch in ['[', '<', '6', '4', ';', '7', '6', ';', '4', '6', 'M'] {
        assert!(matches!(
            consume_mobile_mouse_char(&mut viewport, ch),
            MobileMouseConsume::Consumed
        ));
    }
    assert_eq!(viewport.scroll_top, 18);
    assert!(viewport.mobile_mouse_buffer.is_empty());
}

#[test]
fn mobile_mouse_key_candidate_accepts_alt_prefixed_csi_chars() {
    let viewport = ViewportState::new();
    assert!(is_mobile_mouse_key_candidate(
        &viewport,
        KeyCode::Char('['),
        KeyModifiers::ALT
    ));
    assert!(is_mobile_mouse_key_candidate(
        &viewport,
        KeyCode::Char('7'),
        KeyModifiers::ALT
    ));
    assert!(!is_mobile_mouse_key_candidate(
        &viewport,
        KeyCode::Char('7'),
        KeyModifiers::empty()
    ));
}

#[test]
fn mobile_mouse_key_candidate_accepts_plain_coords_when_pending() {
    let mut viewport = ViewportState::new();
    viewport.mobile_plain_pending_coords = true;
    assert!(is_mobile_mouse_key_candidate(
        &viewport,
        KeyCode::Char('7'),
        KeyModifiers::empty()
    ));
    assert!(is_mobile_mouse_key_candidate(
        &viewport,
        KeyCode::Char(';'),
        KeyModifiers::empty()
    ));
    assert!(!is_mobile_mouse_key_candidate(
        &viewport,
        KeyCode::Char('a'),
        KeyModifiers::empty()
    ));
}

#[test]
fn consume_mobile_mouse_char_plain_pending_pair_applies_scroll() {
    let mut viewport = ViewportState::new();
    viewport.mobile_plain_pending_coords = true;
    viewport.scroll_top = 20;
    viewport.mobile_mouse_last_y = Some(50);

    for ch in ['6', '6', ';', '5', '2'] {
        assert!(matches!(
            consume_mobile_mouse_char(&mut viewport, ch),
            MobileMouseConsume::Consumed
        ));
    }

    assert_eq!(viewport.scroll_top, 23);
    assert!(!viewport.mobile_plain_pending_coords);
    assert!(viewport.mobile_mouse_buffer.is_empty());
}

#[test]
fn consume_mobile_mouse_char_plain_pending_repeated_pair_reuses_direction() {
    let mut viewport = ViewportState::new();
    viewport.scroll_top = 20;
    viewport.mobile_mouse_last_y = Some(50);

    viewport.mobile_plain_pending_coords = true;
    for ch in ['6', '6', ';', '5', '2'] {
        let _ = consume_mobile_mouse_char(&mut viewport, ch);
    }
    assert_eq!(viewport.scroll_top, 23);

    viewport.mobile_plain_pending_coords = true;
    for ch in ['6', '6', ';', '5', '2'] {
        let _ = consume_mobile_mouse_char(&mut viewport, ch);
    }
    assert_eq!(viewport.scroll_top, 26);
}

#[test]
fn consume_mobile_mouse_char_plain_pending_new_gesture_keeps_prior_direction() {
    let mut viewport = ViewportState::new();
    viewport.scroll_top = 20;
    viewport.mobile_mouse_last_y = Some(50);
    viewport.mobile_plain_last_direction = 1;
    viewport.mobile_plain_new_gesture = true;
    viewport.mobile_plain_pending_coords = true;

    for ch in ['6', '4', ';', '4', '7'] {
        let _ = consume_mobile_mouse_char(&mut viewport, ch);
    }

    assert_eq!(viewport.scroll_top, 23);
    assert_eq!(viewport.mobile_plain_last_direction, 1);
}

#[test]
fn parse_repeated_plain_mobile_pair_accepts_concatenated_repetition() {
    assert_eq!(
        parse_repeated_plain_mobile_pair("75;4375;4375;43"),
        Some((75, 43))
    );
    assert_eq!(
        parse_repeated_plain_mobile_pair("71;5371;5371;53"),
        Some((71, 53))
    );
    assert_eq!(parse_repeated_plain_mobile_pair("75;43"), None);
}

#[test]
fn consume_mobile_mouse_char_applies_scroll_on_terminator() {
    let mut viewport = ViewportState::new();
    viewport.scroll_top = 10;
    viewport.mobile_mouse_last_y = Some(40);

    for ch in ['<', '6', '4', ';', '7', '6', ';', '4', '6'] {
        assert!(matches!(
            consume_mobile_mouse_char(&mut viewport, ch),
            MobileMouseConsume::Consumed
        ));
    }
    assert!(matches!(
        consume_mobile_mouse_char(&mut viewport, 'M'),
        MobileMouseConsume::Consumed
    ));
    assert_eq!(viewport.scroll_top, 16);
    assert!(viewport.mobile_mouse_buffer.is_empty());
}

#[test]
fn apply_mobile_mouse_scroll_uses_natural_touch_direction() {
    let mut viewport = ViewportState::new();
    viewport.scroll_top = 20;
    viewport.mobile_mouse_last_y = Some(40);

    apply_mobile_mouse_scroll(&mut viewport, 44);
    assert_eq!(viewport.scroll_top, 24);

    apply_mobile_mouse_scroll(&mut viewport, 42);
    assert_eq!(viewport.scroll_top, 22);
}

#[test]
fn apply_mobile_mouse_scroll_honors_invert_toggle() {
    let mut viewport = ViewportState::new();
    viewport.scroll_inverted = true;
    viewport.scroll_top = 20;
    viewport.mobile_mouse_last_y = Some(40);

    apply_mobile_mouse_scroll(&mut viewport, 44);
    assert_eq!(viewport.scroll_top, 16);

    apply_mobile_mouse_scroll(&mut viewport, 42);
    assert_eq!(viewport.scroll_top, 18);
}

#[test]
fn repeated_pair_parser_keeps_unicode_boundaries_safe() {
    for pasted in ["🦀🦀🦀🦀", "75;4375;43🦀", "日本語の本文です"] {
        assert_eq!(parse_repeated_plain_mobile_pair(pasted), None);
    }
}

#[test]
fn coordinate_detector_does_not_consume_ordinary_pasted_text() {
    let mut viewport = ViewportState::new();
    for text in ["ordinary 66;52 text", "let x = 1; let y = 2;", "🦀 76;46"] {
        assert!(!super::consume_paste(&mut viewport, true, text), "{text}");
    }
}

#[test]
fn parse_mobile_mouse_coords_accepts_plain_and_sgr_fragments() {
    assert_eq!(parse_mobile_mouse_coords("76;46"), Some((76, 46)));
    assert_eq!(
        parse_mobile_mouse_coords("\u{1b}[<64;76;46M"),
        Some((76, 46))
    );
    assert_eq!(parse_mobile_mouse_coords("hello"), None);
}

#[test]
fn consume_mobile_mouse_char_does_not_swallow_plain_digits() {
    let mut app = ViewportState::new();
    assert!(matches!(
        consume_mobile_mouse_char(&mut app, '2'),
        MobileMouseConsume::PassThrough
    ));
    assert!(app.mobile_mouse_buffer.is_empty());
}
