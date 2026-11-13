//! Exercise two independent output watchers without a socket or duplicate session body.

use carlos_proto::commands::RpcVersion;
use carlos_proto::events::{Event, Notification, NotificationMessage};
use carlos_proto::output::OutputUpdate;
use serde_json::{from_str, to_string};

use super::event_tests::synchronize;
use super::models::{MessageKind, Role};
use super::output::OutputReadError;
use super::runtime_settings::RuntimeDefaults;
use super::state::SessionState;

fn receive_chunks(
    published: &super::events::Published<'_>,
    entry: &carlos_proto::ids::EntryId,
    session: &carlos_proto::ids::SessionId,
    cursor: &mut Option<carlos_proto::output::OutputCursor>,
    body: &mut String,
    transferred: &mut usize,
) {
    loop {
        let chunk = published
            .output_chunk(entry, cursor.as_ref(), 8192)
            .expect("chunk");
        let more = chunk.has_more;
        let message = NotificationMessage {
            jsonrpc: RpcVersion::V2,
            notification: Notification::Output(OutputUpdate {
                session: session.clone(),
                chunk,
            }),
        };
        let wire = to_string(&message).expect("output event");
        *transferred += wire.len();
        let decoded: NotificationMessage = from_str(&wire).expect("round trip");
        let Notification::Output(update) = decoded.notification else {
            panic!("output");
        };
        body.extend(update.chunk.segments.into_iter().map(|piece| piece.text));
        *cursor = Some(update.chunk.next);
        if !more {
            break;
        }
    }
}

#[test]
fn expanded_watcher_resumes_while_collapsed_watcher_receives_only_compact_facts() {
    let mut core = SessionState::new("root".into(), RuntimeDefaults::default(), false);
    let mut collapsed = core.publish().snapshot(100);
    let key = core.backend_entry_key("running");
    core.upsert_command_output_delta(&key, "");
    synchronize(&mut core, &mut collapsed);
    let entry = collapsed.entries[0].id.clone();
    let mut cursor = None;
    let mut body = String::new();
    let mut transferred = 0;
    for i in 0..100 {
        core.upsert_command_output_delta(&key, &"🌍".repeat(1024));
        let events = synchronize(&mut core, &mut collapsed);
        let wire = to_string(&events).expect("events");
        assert!(wire.len() < 400, "collapsed frame contains bulk text");
        assert!(events
            .iter()
            .any(|event| matches!(event.event, Event::OutputChanged { .. })));
        if i % 10 != 9 {
            continue;
        } // One watcher pauses between reads, then resumes.
        receive_chunks(
            &core.publish(),
            &entry,
            &collapsed.info.id,
            &mut cursor,
            &mut body,
            &mut transferred,
        );
    }
    assert_eq!(body, "🌍".repeat(1024 * 100));
    assert!(transferred < body.len() * 2);
    core.messages.update(
        0,
        Role::ToolOutput,
        Some("replacement".into()),
        MessageKind::Plain,
        None,
    );
    synchronize(&mut core, &mut collapsed);
    assert_eq!(
        core.publish().output_chunk(&entry, cursor.as_ref(), 1024),
        Err(OutputReadError::Replaced)
    );
    assert_eq!(
        core.publish()
            .output_chunk(&entry, None, 1024)
            .expect("restart")
            .segments[0]
            .text,
        "replacement"
    );
}

#[test]
fn reset_then_edit_and_retired_rows_cannot_reuse_old_output_cursors() {
    let mut core = SessionState::new("root".into(), RuntimeDefaults::default(), false);
    core.append_message(Role::ToolOutput, "body");
    let entry = core.messages[0].id.to_wire();
    let cursor = core
        .publish()
        .output_chunk(&entry, None, 1024)
        .expect("read")
        .next;
    core.append_message(Role::User, "truncate");
    let removed = core.messages[1].id;
    core.messages.truncate_before(removed);
    core.messages.update(
        0,
        Role::ToolOutput,
        Some("new".into()),
        MessageKind::Plain,
        None,
    );
    assert_eq!(
        core.publish().output_chunk(&entry, Some(&cursor), 1024),
        Err(OutputReadError::Replaced)
    );
    core.messages.reset();
    assert_eq!(
        core.publish().output_chunk(&entry, None, 1024),
        Err(OutputReadError::EntryNotFound)
    );
}

#[test]
fn completed_diff_cursors_restart_when_a_source_unexpectedly_reopens() {
    let mut core = SessionState::new("root".into(), RuntimeDefaults::default(), false);
    core.append_message(Role::ToolOutput, "+");
    core.messages.set_kind(0, MessageKind::Diff);
    let id = core.messages[0].id;
    let entry = id.to_wire();
    let chunk = core
        .publish()
        .output_chunk(&entry, None, 1024)
        .expect("read");
    core.messages.activate(id);
    assert!(core
        .publish()
        .output_chunk(&entry, Some(&chunk.next), 1024)
        .is_ok());
    core.messages.update(
        0,
        Role::ToolOutput,
        Some("+++ file".into()),
        MessageKind::Diff,
        None,
    );
    assert_eq!(
        core.publish().output_chunk(&entry, Some(&chunk.next), 1024),
        Err(OutputReadError::Replaced)
    );
}
