//! Replay ANSI controller errors through the plain wrapper and its lazy cache.

use carlos_core::session::entry_id::EntryId;
use carlos_core::session::models::{Message, MessageKind, Role};

use super::{wrap_natural_by_cells, wrap_natural_count_by_cells};
use crate::app::layout_state::TranscriptTheme;
use crate::app::render_cache_state::RenderCacheState;

fn notices() -> Vec<String> {
    serde_json::from_str(include_str!("fixtures/ansi_controller_notices.json"))
        .expect("captured controller notices")
}

#[test]
fn captured_ansi_notice_counts_equal_the_unchanged_painted_rows() {
    for text in notices() {
        for line in text.split('\n') {
            for width in 8..=160 {
                assert_eq!(
                    wrap_natural_count_by_cells(line, width),
                    wrap_natural_by_cells(line, width).len(),
                    "width={width}, line={line:?}"
                );
            }
        }
    }
}

#[test]
fn captured_ansi_notices_materialize_with_their_exact_cached_height() {
    let mut cache = RenderCacheState::new();
    let rows: Vec<_> = notices()
        .into_iter()
        .enumerate()
        .map(|(index, text)| Message {
            id: EntryId::from_wire(
                &format!("{}-{index:016x}", "a".repeat(32))
                    .parse()
                    .expect("entry"),
            ),
            role: Role::System,
            kind: MessageKind::Plain,
            text,
            file_path: None,
            tool_facts: None,
            tool_output: None,
        })
        .collect();
    for width in [28, 72, 99, 100, 120] {
        cache.ensure_inputs(rows.as_slice(), width, None, false, TranscriptTheme::Gutter);
        let count = cache.rendered_line_count();
        cache.materialize_inputs(rows.as_slice(), 0, count.saturating_sub(1));
        assert_eq!(
            (0..count)
                .filter(|row| cache.rendered_line_at(*row).is_some())
                .count(),
            count
        );
    }
}

#[test]
fn plain_ansi_word_wrapping_counts_the_same_raw_cells_as_its_painter() {
    for width in [8, 12, 20] {
        let text = "\x1b[31mfoo\x1b[0m bar";
        assert_eq!(
            wrap_natural_count_by_cells(text, width),
            wrap_natural_by_cells(text, width).len()
        );
    }
}
