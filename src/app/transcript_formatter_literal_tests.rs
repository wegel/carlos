//! Preserve exact literal bytes and retire only the matching shared clipboard permission.

use super::super::{Formatter, TransferError};
use super::{append, MAX_PIECE};

#[test]
fn ordered_literal_pieces_preserve_every_unicode_byte() {
    let mut pending = None;
    let text = "https://host.test/日本語🌍?query=exact".repeat(10000);
    let mut offset = 0;
    while offset < text.len() {
        let mut end = text.len().min(offset + MAX_PIECE);
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        let finished = end == text.len();
        let complete =
            append(&mut pending, 7, offset, &text[offset..end], finished).expect("exact piece");
        if finished {
            assert_eq!(complete.as_deref(), Some(text.as_str()));
        } else {
            assert!(complete.is_none());
        }
        offset = end;
    }
    assert!(pending.is_none());
}

#[test]
fn literal_offsets_duplicates_and_byte_bounds_cannot_change_a_partial_copy() {
    for kind in ["shifted", "duplicate", "too_large", "empty"] {
        let mut pending = None;
        append(&mut pending, 7, 0, "prefix", false).expect("original prefix");
        let (offset, text) = match kind {
            "shifted" => (8, "tail".to_owned()),
            "duplicate" => (0, "prefix".to_owned()),
            "too_large" => (6, "x".repeat(MAX_PIECE + 1)),
            _ => (6, String::new()),
        };
        assert_eq!(
            append(&mut pending, 7, offset, &text, false),
            Err(TransferError::InvalidOffset),
            "{kind}"
        );
        assert!(pending.is_none());
    }
}

#[test]
fn one_clipboard_epoch_retires_literal_and_selection_copy_permissions_together() {
    let mut formatter = Formatter::new().expect("one worker");
    let old = formatter
        .begin_copy()
        .expect("original source or selection permission");
    assert!(formatter.copy_is_current(old));
    let url = formatter.begin_copy().expect("new literal permission");
    assert!(!formatter.copy_is_current(old));
    assert!(formatter.copy_is_current(url));
    assert_eq!(
        formatter.literal_copy(old, 0, "retired URL", true),
        Err(TransferError::Retired)
    );
    assert_eq!(
        formatter.literal_copy(url, 0, &"x".repeat(MAX_PIECE + 1), true),
        Err(TransferError::InvalidOffset)
    );
    formatter.cancel_copy();
    assert!(!formatter.copy_is_current(url));
    assert_eq!(
        formatter.literal_copy(url, 0, "canceled URL", true),
        Err(TransferError::Retired)
    );
}
