//! Check the actual local text, keyboard, status and pointer helpers without a native session.

use carlos_core::session::context_usage::ContextUsage;
use crossterm::event::{KeyEventKind, KeyModifiers};

use super::context_usage::context_usage_label;
use super::notifications::is_key_press_like;
use super::render_input::{is_newline_enter, normalize_pasted_text};
use super::selection::{decide_mouse_drag_mode, MouseDragMode};

#[test]
fn context_usage_label_formats_k_and_percent() {
    let label = context_usage_label(ContextUsage {
        used: 128_000,
        max: 256_000,
    });
    assert_eq!(label, "128k/256k (50%)");
}

#[test]
fn normalize_pasted_text_converts_crlf_and_cr() {
    let text = "a\r\nb\rc";
    assert_eq!(normalize_pasted_text(text), "a\nb\nc");
}

#[test]
fn is_newline_enter_accepts_shift_and_alt() {
    assert!(is_newline_enter(KeyModifiers::SHIFT));
    assert!(is_newline_enter(KeyModifiers::ALT));
    assert!(!is_newline_enter(KeyModifiers::empty()));
}

#[test]
fn is_key_press_like_accepts_repeat() {
    assert!(is_key_press_like(KeyEventKind::Press));
    assert!(is_key_press_like(KeyEventKind::Repeat));
    assert!(!is_key_press_like(KeyEventKind::Release));
}

#[test]
fn decide_mouse_drag_mode_prefers_scroll_for_vertical_swipe() {
    assert_eq!(
        decide_mouse_drag_mode(10, 10, 10, 14),
        MouseDragMode::Scroll
    );
    assert_eq!(
        decide_mouse_drag_mode(10, 10, 12, 14),
        MouseDragMode::Select
    );
    assert_eq!(
        decide_mouse_drag_mode(10, 10, 10, 11),
        MouseDragMode::Select
    );
    assert_eq!(
        decide_mouse_drag_mode(10, 10, 10, 10),
        MouseDragMode::Undecided
    );
}
