//! Compare exported offline inputs against the original core-message renderer and append workload.

use carlos_core::perf_fixture::{
    populate_synthetic_perf_session, synthetic_perf_rows, SyntheticPerfSpec,
};
use carlos_core::session::models::Message;
use carlos_core::session::state::SessionState;

use super::PerfView;
use crate::app::layout_state::TranscriptTheme;
use crate::app::render_cache_state::{RenderCacheState, TranscriptSource};
use crate::app::transcript_input::RenderInput;
use crate::app::view_models::TerminalSize;

fn fixture() -> (Vec<Message>, PerfView) {
    let spec = SyntheticPerfSpec {
        seed: 7,
        turns: 12,
        tool_output_lines: 24,
    };
    let mut original = SessionState::new("original".into(), Default::default(), false);
    populate_synthetic_perf_session(spec, &mut original);
    let messages = original.messages().iter().cloned().collect();
    (messages, PerfView::new(synthetic_perf_rows(spec, |_| {})))
}

#[test]
fn exported_perf_inputs_keep_every_original_and_display_fact() {
    let (original, view) = fixture();
    assert_eq!(original.len(), view.rows.len());
    for (index, message) in original.iter().enumerate() {
        let expected = RenderInput::from(message);
        let actual = view
            .rows
            .as_slice()
            .input(index)
            .expect("complete original");
        assert_eq!(
            format!("{actual:?}"),
            format!("{expected:?}"),
            "row {index}"
        );
    }
}

fn fingerprint<S: TranscriptSource + ?Sized>(
    source: &S,
    width: usize,
    compact: bool,
    theme: TranscriptTheme,
) -> Vec<String> {
    let mut cache = RenderCacheState::new();
    cache.ensure_inputs(source, width, None, compact, theme);
    let count = cache.rendered_line_count();
    if count > 0 {
        cache.materialize_inputs(source, 0, count - 1);
    }
    (0..count)
        .map(|index| format!("{:?}", cache.rendered_line_at(index).expect("painted row")))
        .collect()
}

#[test]
fn exported_perf_rows_keep_wrapping_styles_and_separators_in_both_themes() {
    let (original, view) = fixture();
    for width in [160, 54, 36, 22] {
        for compact in [false, true] {
            for theme in [TranscriptTheme::Gutter, TranscriptTheme::Flow] {
                let expected = fingerprint(original.as_slice(), width, compact, theme);
                let actual = fingerprint(view.rows.as_slice(), width, compact, theme);
                assert_eq!(
                    actual, expected,
                    "width={width} compact={compact} theme={theme:?}",
                );
                println!(
                    "width={width} compact={compact} theme={theme:?} rows={} digest={:016x}",
                    actual.len(),
                    digest(&actual)
                );
            }
        }
    }
}

fn digest(lines: &[String]) -> u64 {
    lines.iter().fold(0xcbf29ce484222325, |hash, line| {
        line.len()
            .to_le_bytes()
            .iter()
            .chain(line.as_bytes())
            .fold(hash, |hash, byte| {
                (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
            })
    })
}

#[test]
fn append_workload_invalidates_the_changed_answer_without_losing_prefix_rows() {
    let (_, mut view) = fixture();
    let size = TerminalSize {
        width: 54,
        height: 18,
    };
    view.view.layout.update_size(size);
    view.view.tool_output.update_size(size);
    let width = view.transcript_width(size);
    view.ensure_rendered_lines(width);
    let before = fingerprint(view.rows.as_slice(), width, true, view.view.layout.active);
    let index = view.append_answer();
    for _ in 0..16 {
        view.append_text(index, "\nappend bench line");
        view.ensure_rendered_lines(width);
    }
    let after = fingerprint(view.rows.as_slice(), width, true, view.view.layout.active);
    assert_eq!(&after[..before.len()], before.as_slice());
    assert_eq!(view.rows[index].text(), "\nappend bench line".repeat(16));
    assert_eq!(view.rendered_line_count(), after.len());
    assert!(after.len() > before.len());
}
