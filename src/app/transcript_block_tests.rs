//! Compare whole core and client blocks using real exported labels, source bodies and spacing.

use carlos_core::session::models::{Role, ToolOutputDetails};
use carlos_core::session::state::SessionState;
use carlos_proto::session::{Entry, LabelSource};
use carlos_proto::source::SourceChunk;

use super::{plan_display_block, plan_input_block};
use crate::app::layout_state::TranscriptTheme;
use crate::app::transcript_input::RenderInput;
use crate::app::transcript_styles::RenderCountCache;
use crate::app::view_models::RenderedLine;

fn fixture() -> SessionState {
    let mut core = SessionState::new("native".into(), Default::default(), false);
    core.append_message(Role::User, "Please inspect the code 🌍");
    core.append_message(Role::Reasoning, "**Thinking**");
    core.append_message(
        Role::Reasoning,
        "The next paragraph contains several words that wrap at a narrow width.",
    );
    let call = core.append_message(Role::ToolCall, "Bash\nprintf 'hello'\n");
    core.transcript_set_tool(
        call,
        ToolOutputDetails {
            summary: "run printf 'hello'".into(),
            generated: false,
            failed: false,
        },
    );
    let output = core.append_message(Role::ToolOutput, "\x1b[31mhello\x1b[0m\nsecond line 🌍");
    core.transcript_set_tool(
        output,
        ToolOutputDetails {
            summary: "A detailed generated tool label that wraps on a phone".into(),
            generated: true,
            failed: true,
        },
    );
    core.append_message(Role::ToolOutput, "unlabelled output\nsecond line");
    core.append_diff_message(
        Role::ToolOutput,
        Some("src/é.rs".into()),
        "@@ -1 +1 @@\n-old\n+new 🌍\n",
    );
    let diff = core.append_diff_message(
        Role::ToolOutput,
        Some("src/é.rs".into()),
        "@@ -1 +1 @@\n-old\n+new 🌍\n",
    );
    core.transcript_set_tool(
        diff,
        ToolOutputDetails {
            summary: "Tool output".into(),
            generated: false,
            failed: false,
        },
    );
    core.append_message(Role::Assistant, "# Done\n\nThe code is ready.");
    core
}

fn exported(core: &mut SessionState) -> (Vec<Entry>, Vec<SourceChunk>) {
    let published = core.publish();
    let entries: Vec<Entry> = published
        .snapshot(100)
        .entries
        .into_iter()
        .map(|entry| {
            serde_json::from_value(serde_json::to_value(entry).expect("wire metadata"))
                .expect("typed client metadata")
        })
        .collect();
    let chunks = entries
        .iter()
        .map(|entry| {
            published
                .source_chunk(&entry.id, None, 65536)
                .expect("actual source")
        })
        .collect();
    (entries, chunks)
}

fn assert_lines(actual: &[RenderedLine], expected: &[RenderedLine]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(
            (
                &actual.text,
                actual.role,
                actual.cells,
                actual.separator,
                actual.soft_wrap_to_next
            ),
            (
                &expected.text,
                expected.role,
                expected.cells,
                expected.separator,
                expected.soft_wrap_to_next
            )
        );
        assert_eq!(actual.styled_segments.len(), expected.styled_segments.len());
        for (actual, expected) in actual.styled_segments.iter().zip(&expected.styled_segments) {
            assert_eq!(
                (&actual.text, actual.style),
                (&expected.text, expected.style)
            );
        }
    }
}

#[test]
fn whole_client_blocks_preserve_core_labels_spacing_counts_and_styles_in_both_themes() {
    let mut core = fixture();
    let (entries, chunks) = exported(&mut core);
    assert_eq!(entries.len(), core.messages().len());
    let inputs: Vec<_> = entries
        .iter()
        .zip(&chunks)
        .map(|(entry, chunk)| {
            RenderInput::from_client(entry, Some((&chunk.source, &chunk.text)))
                .expect("borrowed exported source")
        })
        .collect();
    for theme in [TranscriptTheme::Gutter, TranscriptTheme::Flow] {
        for compact in [false, true] {
            for width in [12, 28, 72, 120] {
                compare_blocks(&core, &inputs, theme, compact, width);
            }
        }
    }
}

fn compare_blocks(
    core: &SessionState,
    inputs: &[RenderInput<'_>],
    theme: TranscriptTheme,
    compact: bool,
    width: usize,
) {
    for (index, (message, input)) in core.messages().iter().zip(inputs).enumerate() {
        let previous = index.checked_sub(1);
        let baseline = plan_display_block(
            &mut RenderCountCache::new(),
            previous.map(|index| &core.messages()[index]),
            message,
            width,
            compact,
            theme,
        );
        let client = plan_input_block(
            &mut RenderCountCache::new(),
            previous.map(|index| inputs[index]),
            *input,
            width,
            compact,
            theme,
        );
        assert_eq!(
            baseline.count(&mut RenderCountCache::new(), message),
            client.count_input(&mut RenderCountCache::new(), *input)
        );
        assert_lines(&client.render_input(*input), &baseline.render(message));
    }
}

#[test]
fn generic_and_saved_labels_remain_distinct_even_when_their_wire_text_is_the_same() {
    let mut core = fixture();
    let (entries, chunks) = exported(&mut core);
    let fallback = &entries[6];
    let saved = &entries[7];
    assert_eq!(
        fallback.tool.as_ref().expect("fallback").label,
        "Tool output"
    );
    assert_eq!(saved.tool.as_ref().expect("saved").label, "Tool output");
    assert_eq!(
        fallback.tool.as_ref().expect("origin").label_source,
        LabelSource::Fallback
    );
    assert_eq!(
        saved.tool.as_ref().expect("origin").label_source,
        LabelSource::Literal
    );
    let input = RenderInput::from_client(fallback, Some((&chunks[6].source, &chunks[6].text)))
        .expect("diff source");
    let block = plan_input_block(
        &mut RenderCountCache::new(),
        None,
        input,
        72,
        true,
        TranscriptTheme::Gutter,
    );
    assert!(block
        .render_input(input)
        .iter()
        .any(|line| line.text.contains("src/é.rs") && line.text.contains("+1")));
    let mut missing = fallback.clone();
    missing.tool.as_mut().expect("label").label_source = LabelSource::Literal;
    let wrong = RenderInput::from_client(&missing, Some((&chunks[6].source, &chunks[6].text)))
        .expect("same bytes, missing origin");
    let block = plan_input_block(
        &mut RenderCountCache::new(),
        None,
        wrong,
        72,
        true,
        TranscriptTheme::Gutter,
    );
    assert!(
        !block
            .render_input(wrong)
            .iter()
            .any(|line| line.text.contains("src/é.rs")),
        "the provenance bit changes an actual rendered result"
    );
}
