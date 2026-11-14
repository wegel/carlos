//! Match paged bodies against every existing body painter while retaining one original allocation.

use std::sync::Arc;

use carlos_core::session::models::{MessageKind, Role};

use super::super::BlockBody;
use super::body::BodyPages;
use crate::app::transcript_input::RenderInput;
use crate::app::view_models::{DisplayKind, RenderedLine};

#[test]
fn every_body_format_keeps_exact_rows_styles_and_original_bytes() {
    let plain = "plain 🌍 text\n```text\nfenced\tcode\n```\nlast  ";
    let markdown = "# Heading\n\n**bold** [link][ref]\n\n| Key | Value |\n| --- | --- |\n| 界 | row |\n\n[ref]: https://example.test\n";
    let ansi = "\x1b[31mred 🌍\nnext\x1b[0m\n";
    let diff = "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -1,2 +1,2 @@\n-old();\n+new();\n context\n";
    let cases = [
        (Role::User, MessageKind::Plain, plain, None),
        (Role::Commentary, MessageKind::Plain, plain, None),
        (Role::Assistant, MessageKind::Plain, markdown, None),
        (Role::Reasoning, MessageKind::Plain, markdown, None),
        (
            Role::ToolCall,
            MessageKind::Plain,
            "```skip\nbody\n```\n",
            None,
        ),
        (
            Role::ToolCall,
            MessageKind::Plain,
            "Bash\nfirst body 🌍\nnext",
            None,
        ),
        (Role::ToolCall, MessageKind::Plain, "Bash", None),
        (Role::ToolOutput, MessageKind::Plain, plain, None),
        (Role::ToolOutput, MessageKind::Plain, ansi, Some("a.rs")),
        (
            Role::ToolOutput,
            MessageKind::Plain,
            "1: /* open\n2: nested\n3: */ fn main() {}\n",
            Some("a.rs"),
        ),
        (Role::ToolOutput, MessageKind::Diff, diff, Some("a.rs")),
        (
            Role::ToolOutput,
            MessageKind::Diff,
            "+unparsed 🌍\n-removed\n```\n",
            Some("a-very-long-unwrapped-source-path.rs"),
        ),
    ];
    for (role, kind, text, path) in cases {
        for width in [8, 12, 28, 120] {
            check_body(role, kind, text, path, width);
        }
    }
}

fn check_body(role: Role, kind: MessageKind, text: &str, path: Option<&str>, width: usize) {
    let input = RenderInput {
        role,
        kind,
        text,
        file_path: path,
        label: None,
        label_generated: false,
        failed: false,
    };
    let body = BlockBody::for_input(input, DisplayKind::Full, width);
    let mut expected = Vec::new();
    body.append_input(&mut expected, input);
    let original = Arc::new(text.to_owned());
    let pointer = Arc::as_ptr(&original);
    let weak = Arc::downgrade(&original);
    let pages = BodyPages::new(original, &body, input).expect("matching existing body format");
    assert_eq!(
        weak.strong_count(),
        1,
        "one original owner after building the index"
    );
    assert_eq!(pages.original(), text);
    assert_eq!(pages.rows(), expected.len(), "{role:?}/{kind:?}/{width}");
    for first in (0..expected.len()).step_by(3) {
        let window = pages.window(first, 7).expect("indexed interval");
        assert_eq!(
            rows(&window),
            rows(&expected[first..(first + 7).min(expected.len())]),
            "{role:?}/{kind:?}/{width}/{first}"
        );
    }
    let original = pages.into_original();
    assert_eq!(
        Arc::as_ptr(&original),
        pointer,
        "return the same allocation for a rebuild"
    );
    assert_eq!(original.as_str(), text);
}

fn rows(lines: &[RenderedLine]) -> Vec<String> {
    lines.iter().map(|line| format!("{line:?}")).collect()
}
