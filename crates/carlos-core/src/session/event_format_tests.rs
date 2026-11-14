//! Preserve source roles, steer/diff formats and paths before a remote renderer consumes entries.

use carlos_proto::events::Event;
use carlos_proto::session::{EntryFormat, EntryKind, ToolStatus};

use super::event_tests::synchronize;
use super::models::{MessageKind, Role, ToolOutputDetails};
use super::state::SessionState;

#[test]
fn snapshots_keep_every_source_role_and_format_without_guessing_from_text() {
    for (role, expected) in [
        (Role::User, EntryKind::User),
        (Role::Assistant, EntryKind::Answer),
        (Role::Commentary, EntryKind::Commentary),
        (Role::Reasoning, EntryKind::Reasoning),
        (Role::ToolCall, EntryKind::ToolCall),
        (Role::ToolOutput, EntryKind::Tool),
        (Role::System, EntryKind::Notice),
    ] {
        for (kind, format) in [
            (MessageKind::Plain, EntryFormat::Plain),
            (MessageKind::UserSteer, EntryFormat::UserSteer),
            (MessageKind::Diff, EntryFormat::Diff),
        ] {
            let mut core = SessionState::new("native".into(), Default::default(), false);
            core.append_message(role, "same original source 🌍");
            core.messages
                .update(0, role, None, kind, Some("src/é.rs".into()));
            let batch = core.publish();
            let row = batch.snapshot(100).entries.remove(0);
            assert_eq!(row.kind, expected);
            assert_eq!(row.format, format);
            assert_eq!(row.file_path.as_deref(), Some("src/é.rs"));
            assert_eq!(
                batch
                    .source_chunk(&row.id, None, 65536)
                    .expect("source")
                    .text,
                "same original source 🌍"
            );
        }
    }
}

#[test]
fn format_only_changes_replace_a_rows_body_when_it_moves_between_inline_and_output() {
    let mut core = SessionState::new("native".into(), Default::default(), false);
    core.append_message(Role::User, "unchanged body");
    let mut client = core.publish().snapshot(100);
    let id = client.entries[0].id.clone();
    core.messages.set_kind(0, MessageKind::Diff);
    let changes = synchronize(&mut core, &mut client);
    assert!(changes
        .iter()
        .any(|event| matches!(event.event, Event::EntryUpdated { .. })));
    assert!(client.entries[0].text.is_empty());
    assert_eq!(client.entries[0].kind, EntryKind::User);
    assert_eq!(client.entries[0].format, EntryFormat::Diff);
    let cursor = core
        .publish()
        .source_chunk(&id, None, 1024)
        .expect("diff source")
        .next;
    core.messages.set_kind(0, MessageKind::Plain);
    synchronize(&mut core, &mut client);
    assert_eq!(client.entries[0].text, "unchanged body");
    assert!(core
        .publish()
        .source_chunk(&id, Some(&cursor), 1024)
        .is_err());
    core.messages.set_kind(0, MessageKind::UserSteer);
    core.messages.set_path(0, Some("other/path".into()));
    let changes = synchronize(&mut core, &mut client);
    assert!(changes
        .iter()
        .any(|event| matches!(event.event, Event::EntryFactsChanged { .. })));
    assert_eq!(client.entries[0].format, EntryFormat::UserSteer);
    assert_eq!(client.entries[0].file_path.as_deref(), Some("other/path"));
    assert_eq!(client.entries[0].text, "unchanged body");
}

#[test]
fn a_native_failure_flag_remains_visible_before_the_completion_boundary() {
    let mut core = SessionState::new("native".into(), Default::default(), false);
    core.append_message(Role::ToolOutput, "failed partial output");
    core.messages.activate(core.messages[0].id);
    core.messages.set_tool(
        0,
        ToolOutputDetails {
            generated: false,
            summary: "Run command".into(),
            failed: true,
        },
    );
    let row = core.publish().snapshot(100).entries.remove(0);
    assert!(!row.done);
    assert_eq!(row.tool.expect("tool facts").status, ToolStatus::Failed);
}
