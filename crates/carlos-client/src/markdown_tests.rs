//! Keep whole-document references, nested styles, lists, quoted code and table cells intact.

use crate::markdown::{parse, Block, BlockKind, Column};

fn text(block: &Block) -> String {
    block.spans.iter().map(|span| span.text.as_str()).collect()
}

#[test]
fn late_reference_definitions_resolve_before_native_rendering_and_nested_styles_restore() {
    let blocks = parse(
        "A [late][id] **bold _both_ still bold** `x` ~~gone~~.\n\n[id]: https://example.test/é\n",
    )
    .expect("Markdown");
    assert_eq!(blocks.len(), 1);
    assert_eq!(text(&blocks[0]), "A late bold both still bold x gone.");
    let spans = &blocks[0].spans;
    assert_eq!(
        spans
            .iter()
            .find(|s| s.text == "late")
            .expect("link")
            .style
            .link
            .as_deref(),
        Some("https://example.test/é")
    );
    let both = spans
        .iter()
        .find(|s| s.text == "both")
        .expect("nested style");
    assert!(both.style.strong && both.style.emphasis);
    let following = spans
        .iter()
        .find(|s| s.text == " still bold")
        .expect("restored style");
    assert!(following.style.strong && !following.style.emphasis);
    assert!(
        spans
            .iter()
            .find(|s| s.text == "x")
            .expect("code")
            .style
            .code
    );
    assert!(
        spans
            .iter()
            .find(|s| s.text == "gone")
            .expect("strike")
            .style
            .strike
    );
}

#[test]
fn nested_lists_keep_original_numbers_and_quoted_code_keeps_its_exact_bytes() {
    let blocks =
        parse("3. first\n   - nested\n4. second\n\n> ```rust\n> let crab = \"🦀\";\n> ```\n")
            .expect("Markdown");
    assert_eq!(blocks[0].marker.as_deref(), Some("3."));
    assert_eq!(blocks[1].marker.as_deref(), Some("•"));
    assert_eq!(blocks[1].indent, 2);
    assert_eq!(blocks[2].marker.as_deref(), Some("4."));
    assert_eq!(blocks[3].quote_depth, 1);
    assert_eq!(
        blocks[3].kind,
        BlockKind::Code {
            language: Some("rust".into())
        }
    );
    assert_eq!(text(&blocks[3]), "let crab = \"🦀\";\n");
}

#[test]
fn table_rows_keep_cells_alignment_and_inline_styles_without_executing_html() {
    let blocks =
        parse("| left | right |\n| :--- | ---: |\n| **🌍** | `x` |\n\n<script>no()</script>\n")
            .expect("Markdown");
    assert_eq!(
        blocks[0].kind,
        BlockKind::TableRow {
            header: true,
            columns: vec![Column::Left, Column::Right]
        }
    );
    assert_eq!(blocks[1].cells[0][0].text, "🌍");
    assert!(blocks[1].cells[0][0].style.strong);
    assert!(blocks[1].cells[1][0].style.code);
    assert_eq!(text(&blocks[2]), "<script>no()</script>\n");
}

#[test]
fn paragraph_soft_breaks_wrap_normally_and_explicit_hard_breaks_remain_newlines() {
    let blocks = parse("soft\nwrap  \nhard").expect("Markdown");
    assert_eq!(text(&blocks[0]), "soft wrap\nhard");
}
