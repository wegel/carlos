//! Compare complete viewport blocks with the current headings, fold rules and reasoning marker.

use std::sync::Arc;

use carlos_core::session::models::{MessageKind, Role};

use super::frame::{Facts, FormattedBlock, View};
use crate::app::flow_transcript::{inline_reasoning, EntryContext};
use crate::app::layout_state::TranscriptTheme;
use crate::app::render_entry_cache::RowFormat;
use crate::app::transcript_block::plan_context_block;
use crate::app::transcript_input::RenderInput;
use crate::app::transcript_styles::RenderCountCache;

fn input(role: Role, text: &str) -> RenderInput<'_> {
    RenderInput {
        role,
        kind: MessageKind::Plain,
        text,
        file_path: None,
        label: None,
        label_generated: false,
        failed: false,
    }
}

#[test]
fn complete_paged_blocks_match_existing_headers_labels_markers_and_folds() {
    let long = "\x1b[31mred line 🌍\x1b[0m\n".repeat(30);
    let cases = [
        input(Role::User, "prompt 🌍"),
        input(Role::Reasoning, "**Thinking**"),
        input(
            Role::Reasoning,
            "more detailed **reasoning** words that wrap at a phone width",
        ),
        input(
            Role::Assistant,
            "# Answer\n\n**bold** [link][ref]\n\n[ref]: https://example.test",
        ),
        input(Role::ToolCall, "Bash\nprintf hello"),
        input(Role::ToolOutput, &long),
    ];
    for mut input in cases {
        input.label =
            matches!(input.role, Role::ToolCall | Role::ToolOutput).then_some("run printf hello");
        for theme in [TranscriptTheme::Gutter, TranscriptTheme::Flow] {
            for width in [8, 12, 28, 72, 120] {
                for compact in [false, true] {
                    check_block(input, width, compact, theme);
                }
            }
        }
    }
}

fn check_block(input: RenderInput<'_>, width: usize, compact: bool, theme: TranscriptTheme) {
    let previous = Some(EntryContext::new(
        self::input(Role::User, "previous"),
        width,
        theme,
    ));
    let context = EntryContext::new(input, width, theme);
    let plan = plan_context_block(
        &mut RenderCountCache::new(),
        previous,
        context,
        input,
        width,
        compact,
        theme,
    );
    let expected = plan.render_input(input);
    let view = View {
        facts: Facts::from_input(input),
        format: RowFormat {
            width,
            compact,
            theme,
            hidden: false,
        },
        previous,
        first: 0,
        length: 50,
    };
    let block = FormattedBlock::new(Arc::new(input.text.into()), &view).expect("complete block");
    assert_eq!(block.original(), input.text);
    check_requested(&block, &view, expected.len());
    for first in (0..expected.len()).step_by(3) {
        let frame = block.window(first, 7).expect("viewport");
        assert_eq!(
            frame.count,
            expected.len(),
            "{:?}/{theme:?}/{width}/{compact}",
            input.role
        );
        let painted = frame.lines.expect("painted window");
        for (index, line) in expected.iter().enumerate().skip(first).take(7) {
            assert_eq!(
                format!("{:?}", painted.get(index).expect("absolute row")),
                format!("{line:?}"),
                "{:?}/{theme:?}/{width}/{compact}/{index}",
                input.role
            );
        }
    }
    assert_eq!(block.into_original().as_str(), input.text);
}

fn check_requested(block: &FormattedBlock, view: &View, total: usize) {
    let requested = block
        .window(view.first, view.length)
        .expect("requested viewport");
    assert_eq!(requested.count, total);
    let painted = requested.lines.expect("requested painted rows");
    if total > view.length {
        assert!(
            painted.get(view.length).is_none(),
            "do not paint the next viewport"
        );
    }
}

#[test]
fn cached_flow_inline_choices_match_the_existing_reasoning_marker_rule() {
    for text in [
        "**Thinking**",
        "**more words that wrap**",
        "# Header",
        "\nshort\n",
        "e\u{301}界🙂",
        "[ref]: https://example.test",
    ] {
        for width in [8, 10, 12, 28, 120] {
            let input = input(Role::Reasoning, text);
            let context = EntryContext::new(input, width, TranscriptTheme::Flow);
            assert_eq!(
                context.is_inline(),
                inline_reasoning(input, width),
                "{text}/{width}"
            );
        }
    }
}
