//! Keep private folding choices while accepted reset frames replace and clear shared rows.

use carlos_core::session::models::{EntryDraft, MessageKind, Role, ToolOutputDetails};
use carlos_proto::events::Event;
use carlos_proto::ids::Sequence;
use carlos_proto::session::Snapshot;

use super::{accept_event, all_rows, draw, scrolled_inline_device};
use crate::app::tests::render_fixture;
use crate::app::tool_output_state::ToolOutputMode;

#[test]
fn reload_and_fork_keep_the_preference_but_replace_message_contents() {
    let (mut device, mut link, _peer) = scrolled_inline_device();
    device.view.tool_output.mode = ToolOutputMode::Compact;
    device.view.viewport.auto_follow_bottom = true;
    let row = render_fixture::draft(EntryDraft {
        role: Role::ToolOutput,
        kind: MessageKind::Plain,
        text: (0..12)
            .map(|i| format!("source line {i}"))
            .collect::<Vec<_>>()
            .join("\n"),
        file_path: None,
        tool_output: Some(ToolOutputDetails {
            generated: false,
            summary: "→ Read main.rs".into(),
            failed: false,
        }),
    });
    let replaced = reset(&device, 2, vec![row.entry.clone()]);
    accept_event(
        &mut device,
        &mut link,
        2,
        Event::Reset {
            snapshot: Box::new(replaced),
        },
    );
    crate::app::remote::widget_fixture::install_source(&mut device, row);
    draw(&mut device, 100, 32);
    let transcript = all_rows(&mut device)
        .iter()
        .map(|row| row.text.clone())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(transcript.trim_end().ends_with("12"));
    assert!(!transcript.contains("hello"));
    let forked = reset(&device, 3, Vec::new());
    accept_event(
        &mut device,
        &mut link,
        3,
        Event::Reset {
            snapshot: Box::new(forked),
        },
    );
    draw(&mut device, 100, 32);
    assert_eq!(device.view.tool_output.mode, ToolOutputMode::Compact);
    assert_eq!(
        device
            .client
            .session(&device.session)
            .expect("forked facts")
            .entry_count(),
        0
    );
    assert_eq!(device.view.render_cache.rendered_line_count(), 0);
}

fn reset(device: &super::Device, seq: u64, entries: Vec<carlos_proto::session::Entry>) -> Snapshot {
    Snapshot {
        info: device
            .client
            .session(&device.session)
            .expect("same logical session")
            .info()
            .clone(),
        entries,
        approvals: Vec::new(),
        seq: Sequence::new(seq),
        has_older: false,
        goal: None,
        queue: None,
        settings: None,
        tasks: None,
    }
}
