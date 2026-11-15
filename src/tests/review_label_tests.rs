//! Paint the generated-label marker independently from native command ownership.

use super::render_fixture;
use crate::app::layout_state::TranscriptTheme;
use crate::app::tool_output_state::ToolOutputMode;
use carlos_core::session::models::{EntryDraft, MessageKind, Role, ToolOutputDetails};

#[test]
fn generated_native_tool_label_remains_visible_in_full_gutter_rows() {
    let mut view = render_fixture::view(vec![render_fixture::draft(EntryDraft {
        role: Role::ToolOutput,
        kind: MessageKind::Plain,
        text: "body".into(),
        file_path: None,
        tool_output: Some(ToolOutputDetails {
            summary: "run native".into(),
            generated: true,
            failed: false,
        }),
    })]);
    view.view.tool_output.mode = ToolOutputMode::Full;
    view.view.layout.active = TranscriptTheme::Gutter;
    let current = render_fixture::snapshot(&mut view, 80);
    assert!(current.iter().any(|line| line.text == "native"));
    assert!(current.iter().any(|line| line.text == "body"));
}
