//! Verify tab-range reads through bounded access to separately retained strings.

use super::SourceRanges;
use crate::app::text::expand_tabs;

#[test]
fn a_source_accessor_can_rebuild_cross_span_rows_after_the_flat_temporary_is_dropped() {
    let spans = ["ab".to_string(), "\t界👩".to_string(), "‍💻z   ".to_string()];
    let flat = spans.concat();
    let expected = expand_tabs(&flat);
    let mut ranges = SourceRanges::default();
    let original = 0..flat.len();
    let expanded = ranges.append_line(std::slice::from_ref(&original), &flat, &flat);
    assert_eq!(expanded, expected);
    drop(flat);
    drop(expanded);
    for start in 0..expected.len() {
        for end in start..expected.len() {
            if !expected.is_char_boundary(start) || !expected.is_char_boundary(end) {
                continue;
            }
            let mut copied = 0;
            let actual = ranges.text_with(start..end, |wanted, output| {
                let mut offset = 0;
                for span in &spans {
                    let end = offset + span.len();
                    let from = wanted.start.max(offset);
                    let until = wanted.end.min(end);
                    if from < until {
                        output.push_str(&span[from - offset..until - offset]);
                        copied += until - from;
                    }
                    offset = end;
                }
            });
            assert_eq!(actual, expected[start..end]);
            assert!(copied <= end - start);
        }
    }
}
