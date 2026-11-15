//! Paint late tool labels without owning their native requests or mutable session.

use super::render_fixture;
use crate::app::tool_output_state::ToolOutputMode;
use carlos_core::session::models::{EntryDraft, MessageKind, Role, ToolOutputDetails};

#[test]
fn late_generated_labels_keep_the_original_failed_output_visible() {
    for (label, generated) in [
        ("run python3 process.py", false),
        ("run Rewrite the report", true),
    ] {
        let mut view = render_fixture::view(vec![render_fixture::draft(EntryDraft {
            role: Role::ToolOutput,
            kind: MessageKind::Plain,
            text: "original output".into(),
            file_path: None,
            tool_output: Some(ToolOutputDetails {
                summary: label.into(),
                generated,
                failed: true,
            }),
        })]);
        view.view.tool_output.mode = ToolOutputMode::Full;
        let visible = render_fixture::snapshot(&mut view, 100);
        assert!(visible.iter().any(|line| line
            .text
            .contains(label.strip_prefix("run ").expect("runner label"))));
        assert!(visible
            .iter()
            .any(|line| line.text.contains("original output")));
    }
}
