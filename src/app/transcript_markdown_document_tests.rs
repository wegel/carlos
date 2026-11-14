//! Compare parsed-document windows with complete Markdown rendering, including late references.

use carlos_core::session::models::Role;

use super::DocumentFrame;
use crate::app::transcript_styles::append_wrapped_markdown_lines;
use crate::app::view_models::RenderedLine;

#[test]
fn a_whole_markdown_document_keeps_late_references_tables_and_nested_constructs() {
    let unit = "# Heading 🌍\n\n**bold** and *italic* [linked][ref] `code`\n\n\
        - first\n  - nested 🙂\n\n> quote **inside**\n\n\
        | Label | Value |\n| --- | --- |\n| one | long text 界 |\n\n\
        ```text\n**literal** [ref] 🌍\n```\n\n";
    let source = unit.repeat(150) + "[ref]: https://example.test\n";
    for width in [8, 12, 28, 80, 120] {
        let mut expected = Vec::new();
        append_wrapped_markdown_lines(&mut expected, Role::Assistant, &source, width);
        let mut document = DocumentFrame::parse(&source, width);
        document.index();
        assert_eq!(document.rows(), expected.len());
        for start in (0..expected.len()).step_by(37) {
            assert_eq!(
                rows(&document.window(start, 53)),
                rows(&expected[start..(start + 53).min(expected.len())]),
                "width{width},start{start}"
            );
        }
    }
}

fn rows(lines: &[RenderedLine]) -> Vec<String> {
    lines.iter().map(|line| format!("{line:?}")).collect()
}

#[test]
fn long_paragraphs_and_code_lines_keep_exact_text_styles_and_wraps_for_both_markdown_roles() {
    let paragraph = "**styled** [link][ref] 界e\u{301}🙂 words\t ".repeat(750);
    let code = "```text\n".to_owned() + &"raw\t🌍 text trailing  ".repeat(1100) + "\n```\n";
    let cases = [paragraph + "\n\n[ref]: https://example.test\n", code];
    for role in [Role::Assistant, Role::Reasoning] {
        for source in &cases {
            for width in [8, 28, 120] {
                let mut expected = Vec::new();
                append_wrapped_markdown_lines(&mut expected, role, source, width);
                let mut document = DocumentFrame::parse_role(role, source, width);
                document.index();
                assert!(
                    !document.long.is_empty(),
                    "exercise saved within-line ranges"
                );
                assert_eq!(document.rows(), expected.len(), "{role:?}/{width}");
                for start in (0..expected.len()).step_by(43) {
                    assert_eq!(
                        rows(&document.window(start, 57)),
                        rows(&expected[start..(start + 57).min(expected.len())]),
                        "{role:?}/{width}/{start}"
                    );
                }
            }
        }
    }
}
