//! Compare client-owned whole bodies with existing rendering and expose lost suffix parser state.

use carlos_core::session::entry_id::EntryId;
use carlos_core::session::models::{Message, MessageKind, Role};
use carlos_core::session::state::SessionState;
use carlos_proto::bytes::ByteLength;
use carlos_proto::output::OutputRevision;
use carlos_proto::session::{Entry, EntryFormat, EntryKind};
use carlos_proto::source::Source;
use serde_json::json;

use super::BlockBody;
use crate::app::transcript_input::RenderInput;
use crate::app::transcript_styles::RenderCountCache;
use crate::app::view_models::{DisplayKind, DisplayMessage, RenderedLine};

fn entry(kind: EntryKind, format: EntryFormat, text: &str, path: Option<&str>) -> Entry {
    Entry {
        id: format!("{}-0000000000000001", "a".repeat(32))
            .parse()
            .expect("entry"),
        kind,
        format,
        file_path: path.map(str::to_owned),
        text: String::new(),
        source: Some(Source {
            revision: OutputRevision::new(1),
            bytes: ByteLength::new(text.len() as u64),
        }),
        done: true,
        tool: None,
        operation: None,
    }
}

fn message(entry: &Entry, text: String, role: Role, kind: MessageKind) -> Message {
    Message {
        id: EntryId::from_wire(&entry.id),
        role,
        kind,
        file_path: entry.file_path.clone(),
        text,
        tool_facts: None,
        tool_output: None,
    }
}

fn assert_lines(actual: &[RenderedLine], expected: &[RenderedLine]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(
            (
                &actual.text,
                actual.cells,
                actual.role,
                actual.separator,
                actual.soft_wrap_to_next
            ),
            (
                &expected.text,
                expected.cells,
                expected.role,
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
fn borrowed_client_bodies_preserve_complete_markdown_ansi_diff_source_and_wrapping() {
    let cases = [
        (EntryKind::Answer,EntryFormat::Plain,Role::Assistant,MessageKind::Plain,None,"# Heading\n\n[link][ref]\n\n| key | value |\n| --- | --- |\n| 🌍 | **bold** |\n\n```rs\nlet a = 1;\n```\n\n[ref]: https://example.test\n"),
        (EntryKind::Reasoning,EntryFormat::Plain,Role::Reasoning,MessageKind::Plain,None,"**Thinking**\n\n| A | B |\n| - | - |\n| hello | world |"),
        (EntryKind::Tool,EntryFormat::Plain,Role::ToolOutput,MessageKind::Plain,None,"\x1b[31mred 🌍\nsecond line\x1b[0m\n"),
        (EntryKind::Tool,EntryFormat::Plain,Role::ToolOutput,MessageKind::Plain,Some("src/main.rs"),"/* comment\nstill comment */\nfn main() { let s = \"🌍\"; }"),
        (EntryKind::Tool,EntryFormat::Diff,Role::ToolOutput,MessageKind::Diff,Some("src/main.rs"),"@@ -1 +1 @@\n-old\n+new 🌍\n"),
        (EntryKind::User,EntryFormat::UserSteer,Role::User,MessageKind::UserSteer,None,"steer\n```text\nuser fence\n```\n"),
        (EntryKind::ToolCall,EntryFormat::Plain,Role::ToolCall,MessageKind::Plain,None,"Bash\nprintf 'hello'\n"),
    ];
    for (kind, format, role, message_kind, path, text) in cases {
        let entry = entry(kind, format, text, path);
        let message = message(&entry, text.into(), role, message_kind);
        let input = RenderInput::from_client(
            &entry,
            Some((entry.source.as_ref().expect("source"), &message.text)),
        )
        .expect("client body");
        assert_eq!(
            input.text.as_ptr(),
            message.text.as_ptr(),
            "adapter borrows the one body"
        );
        for width in [28, 72, 120] {
            let baseline = BlockBody::new(DisplayMessage::full(&message), width);
            let candidate = BlockBody::for_input(input, DisplayKind::Full, width);
            assert_eq!(
                baseline.count(&mut RenderCountCache::new(), &message),
                candidate.count_input(&mut RenderCountCache::new(), input)
            );
            let (mut old, mut new) = (Vec::new(), Vec::new());
            baseline.append(&mut old, &message);
            candidate.append_input(&mut new, input);
            assert_lines(&new, &old);
        }
    }
}

#[test]
fn a_client_suffix_or_replaced_revision_cannot_pose_as_a_complete_document() {
    let text = format!("```text\n{}\n```", "**literal 🌍**\n".repeat(24000));
    let entry = entry(EntryKind::Answer, EntryFormat::Plain, &text, None);
    let source = entry.source.as_ref().expect("source");
    assert!(text.len() > 256 * 1024);
    assert!(RenderInput::from_client(&entry, Some((source, "```"))).is_none());
    let mut retired = source.clone();
    retired.revision = OutputRevision::new(0);
    assert!(RenderInput::from_client(&entry, Some((&retired, &text))).is_none());
    assert!(RenderInput::from_client(&entry, None).is_none());
}

#[test]
fn a_missing_prefix_changes_ansi_colors_even_when_the_visible_tail_text_is_identical() {
    let text = format!("\x1b[31m{}tail is red", "coloured line\n".repeat(24000));
    let full = RenderInput {
        role: Role::ToolOutput,
        kind: MessageKind::Plain,
        text: &text,
        file_path: None,
        label: None,
        label_generated: false,
        failed: false,
    };
    let suffix = RenderInput {
        text: "tail is red",
        ..full
    };
    let (mut old, mut truncated) = (Vec::new(), Vec::new());
    BlockBody::for_input(full, DisplayKind::Full, 72).append_input(&mut old, full);
    BlockBody::for_input(suffix, DisplayKind::Full, 72).append_input(&mut truncated, suffix);
    let last = old.last().expect("full tail");
    assert_eq!(last.text, truncated[0].text);
    assert_ne!(
        last.styled_segments[0].style,
        truncated[0].styled_segments[0].style
    );
}

#[test]
fn tool_output_requires_the_matching_native_source_revision_even_without_a_row_descriptor() {
    let text = "\x1b[31moriginal output";
    let mut entry = entry(EntryKind::Tool, EntryFormat::Plain, text, None);
    let mut source = entry.source.take().expect("source");
    entry.tool = Some(
        serde_json::from_value(json!({"kind":"command","label":"run echo",
        "label_source":"literal","status":"running","summary":null,"added":null,"removed":null,
        "has_output":true,"output_revision":"1"}))
        .expect("wire tool"),
    );
    assert!(
        RenderInput::from_client(&entry, None).is_none(),
        "empty row text is not an empty tool body"
    );
    let body =
        RenderInput::from_client(&entry, Some((&source, text))).expect("current complete output");
    assert_eq!(body.text, text);
    assert_eq!(body.role, Role::ToolOutput);
    source.revision = OutputRevision::new(0);
    assert!(RenderInput::from_client(&entry, Some((&source, text))).is_none());
}

#[test]
fn an_actual_exported_tool_call_fetches_source_even_when_it_has_no_output() {
    let mut core = SessionState::new("native".into(), Default::default(), false);
    core.append_message(Role::ToolCall, "Bash\nprintf 'hello 🌍'\n");
    let (entry, chunk) = {
        let published = core.publish();
        let entry = published.snapshot(100).entries.remove(0);
        let chunk = published
            .source_chunk(&entry.id, None, 65536)
            .expect("actual source");
        (entry, chunk)
    };
    assert!(entry.text.is_empty());
    assert!(entry.source.is_none());
    assert!(!entry.tool.as_ref().expect("tool call facts").has_output);
    assert!(RenderInput::from_client(&entry, None).is_none());
    let input = RenderInput::from_client(&entry, Some((&chunk.source, &chunk.text)))
        .expect("actual tool-call source");
    let original = &core.messages()[0];
    assert_eq!(input.text, original.text);
    for width in [28, 72, 120] {
        let baseline = BlockBody::new(DisplayMessage::full(original), width);
        let candidate = BlockBody::for_input(input, DisplayKind::Full, width);
        let count = baseline.count(&mut RenderCountCache::new(), original);
        assert!(count > 0);
        assert_eq!(
            candidate.count_input(&mut RenderCountCache::new(), input),
            count
        );
        let (mut old, mut new) = (Vec::new(), Vec::new());
        baseline.append(&mut old, original);
        candidate.append_input(&mut new, input);
        assert_lines(&new, &old);
    }
}
