//! Compare plain windows against complete messages, including fence and trailing-line rules.

use std::sync::Arc;

use carlos_core::session::models::Role;

use super::super::PageError;
use super::PlainPages;
use crate::app::transcript_styles::append_wrapped_message_lines;
use crate::app::view_models::RenderedLine;

#[test]
fn plain_windows_preserve_fences_empty_lines_unicode_and_exact_styles() {
    let cases = [
        String::new(),
        "\n".to_string(),
        "```rs\nlet x = 1;\n\n```\nlast\n".to_string(),
        " before\rwords\t界 👩‍💻 é  \n  ```  \r\ncode\n  ```\r\n".repeat(200),
        "words [a-b]: 界👩‍💻é\t end  ".repeat(1000),
        "```\n".to_string() + &"界👩‍💻é\t; words  ".repeat(1200) + "\n```\n",
    ];
    for source in cases {
        for role in [
            Role::User,
            Role::Assistant,
            Role::ToolOutput,
            Role::Reasoning,
        ] {
            for width in [8, 12, 28, 80, 120] {
                let pages =
                    PlainPages::new(Arc::new(source.clone()), role, width).expect("plain index");
                assert_eq!(pages.original_text(), source);
                let mut expected = Vec::new();
                append_wrapped_message_lines(&mut expected, role, &source, width);
                assert_eq!(pages.rows(), expected.len(), "role{role:?},width{width}");
                for start in (0..expected.len()).step_by(137) {
                    let end = (start + 37).min(expected.len());
                    assert_eq!(
                        rows(&pages.window(start, 37)),
                        rows(&expected[start..end]),
                        "role{role:?},width{width},start{start}"
                    );
                }
                assert!(pages.index_capacity_bytes() < source.len() * 8 + 1024);
            }
        }
    }
}

#[test]
fn appending_and_resizing_plain_text_keeps_fence_state_and_the_final_empty_line() {
    for role in [Role::User, Role::Assistant] {
        let mut source = "ordinary row\n".repeat(129) + "```\ncode";
        let mut pages = PlainPages::new(Arc::new(source.clone()), role, 28).expect("plain index");
        for addition in [" words", "\n", "界é\t", "\n```", "\nlast\n"] {
            source.push_str(addition);
            pages.append(addition);
            for width in [28, 8, 120] {
                pages = pages.resize(width).expect("supported width");
                let mut expected = Vec::new();
                append_wrapped_message_lines(&mut expected, role, &source, width);
                assert_eq!(pages.rows(), expected.len());
                assert_eq!(rows(&pages.window(0, pages.rows())), rows(&expected));
            }
        }
    }
}

fn rows(lines: &[RenderedLine]) -> Vec<String> {
    lines.iter().map(|line| format!("{line:?}")).collect()
}

#[test]
fn consuming_a_plain_index_returns_the_existing_original_owner() {
    let source = Arc::new("plain original".to_string());
    let pointer = Arc::as_ptr(&source);
    let pages = PlainPages::new(source, Role::User, 28).expect("plain index");
    let source = pages.into_original();
    assert_eq!(Arc::as_ptr(&source), pointer);
    assert_eq!(Arc::strong_count(&source), 1);
    assert_eq!(source.as_str(), "plain original");
}

#[test]
fn suffix_views_ignore_a_skipped_fence_header_and_keep_the_full_original() {
    let mut source = "```\n".to_string() + &"body words 界é\t ".repeat(1400) + "\n";
    let mut pages = PlainPages::from_offset(Arc::new(source.clone()), Role::User, 28, 4)
        .expect("body suffix starts after the skipped fence header");
    for addition in ["", "next\n```\nfenced body"] {
        source.push_str(addition);
        pages.append(addition);
        for width in [28, 8, 120] {
            pages = pages.resize(width).expect("suffix at another width");
            let mut expected = Vec::new();
            append_wrapped_message_lines(&mut expected, Role::User, &source[4..], width);
            assert_eq!(rows(&pages.window(0, pages.rows())), rows(&expected));
            assert_eq!(pages.original_text(), source);
        }
    }
    for offset in [1, 2, 100] {
        let error = PlainPages::from_offset(Arc::new("界".to_string()), Role::User, 28, offset)
            .err()
            .expect("offset must be in bounds and on a UTF-8 boundary");
        assert_eq!(error, PageError::InvalidOffset);
    }
}
