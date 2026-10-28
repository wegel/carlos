//! Check that the phone scanner crosses status text without replacing characters.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use super::sweep_status_line;
use crate::theme::{kitt_color_for_distance, COLOR_DIFF_REMOVE, COLOR_STEP2, COLOR_TEXT};

#[test]
fn phone_sweep_crosses_text_and_preserves_symbols_backgrounds_and_modifiers() {
    for ralph in [false, true] {
        let mut original = Buffer::empty(Rect::new(0, 0, 54, 3));
        original.set_string(
            0,
            1,
            "━━━━ gpt-model/high 91k/100k 91%",
            Style::default()
                .fg(COLOR_TEXT)
                .bg(COLOR_STEP2)
                .add_modifier(Modifier::ITALIC),
        );
        original[(28, 1)].fg = COLOR_DIFF_REMOVE;
        for head in 0..53 {
            let mut frame = original.clone();
            sweep_status_line(&mut frame, 1, 53, ralph, head as u128);
            for x in 0..54 {
                let before = &original[(x, 1)];
                let after = &frame[(x, 1)];
                assert_eq!(after.symbol(), before.symbol());
                assert_eq!(after.bg, before.bg);
                let expected_modifiers = if x < 53 && x == head {
                    before.modifier | Modifier::BOLD
                } else {
                    before.modifier
                };
                assert_eq!(after.modifier, expected_modifiers);
                let distance = (head as usize).abs_diff(x as usize);
                let expected = if x < 53 && distance <= 3 {
                    kitt_color_for_distance(distance, ralph)
                } else {
                    before.fg
                };
                assert_eq!(after.fg, expected);
                assert_eq!(frame[(x, 0)], original[(x, 0)]);
                assert_eq!(frame[(x, 2)], original[(x, 2)]);
            }
        }
    }
}

#[test]
fn phone_sweep_handles_empty_and_single_column_lines() {
    let mut frame = Buffer::empty(Rect::new(0, 0, 1, 1));
    let original = frame.clone();
    sweep_status_line(&mut frame, 0, 0, false, 7);
    assert_eq!(frame, original);
    frame[(0, 0)].set_symbol("界");
    sweep_status_line(&mut frame, 0, 1, false, 7);
    assert_eq!(frame[(0, 0)].symbol(), "界");
    assert_eq!(frame[(0, 0)].fg, kitt_color_for_distance(0, false));
}
