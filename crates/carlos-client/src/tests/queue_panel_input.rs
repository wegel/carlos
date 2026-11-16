//! Keep complete native input reads private, bounded and independent across devices.

use carlos_proto::bytes::ByteLength;
use carlos_proto::commands::{Command, RpcError};
use carlos_proto::events::{Event, Notification};
use serde_json::json;

use super::client::{attached, command, notify, reply};
use super::queue_panel::{id, page, single_read};
use super::{event, session};
use crate::codec::Incoming;
use crate::panels::QueueReadMode;
use crate::{Client, ClientError, ConnectionId, Effects, Outbound};

#[test]
fn clients_receive_complete_utf8_text_without_sharing_drafts_or_retaining_a_second_copy() {
    let text = format!("{}🌍\nlast line", "a".repeat(65535));
    let (mut first, _, first_read) = reading(text.len() as u64);
    let (mut second, _, second_read) = reading(text.len() as u64);
    let first_chunk = text_reply(&mut first, single_read(&first_read), &text[..65535], false);
    assert!(first.take_queue_input(&session()).is_none());
    assert!(first
        .queue_panel(&session())
        .expect("panel")
        .is_reading_input());
    let last = text_reply(&mut first, single_read(&first_chunk), &text[65535..], true);
    assert!(last.outgoing.is_empty());
    let mut owned = first
        .take_queue_input(&session())
        .expect("whole checked input");
    assert_eq!(owned.text, text);
    assert_eq!(owned.entry, id(0));
    assert!(first.take_queue_input(&session()).is_none());
    assert!(second.take_queue_input(&session()).is_none());
    owned.text.push_str(" my private draft");
    let second_chunk = text_reply(
        &mut second,
        single_read(&second_read),
        &text[..65535],
        false,
    );
    text_reply(
        &mut second,
        single_read(&second_chunk),
        &text[65535..],
        true,
    );
    assert_eq!(
        second
            .take_queue_input(&session())
            .expect("independent input")
            .text,
        text
    );
    assert!(owned.text.ends_with("my private draft"));
}

#[test]
fn readers_reject_a_short_final_chunk_and_keep_input_failures_separate_from_page_errors() {
    let (mut client, _, read) = reading(12);
    let failed = text_reply(&mut client, single_read(&read), "short", true);
    assert!(failed.outgoing.is_empty());
    assert!(client.take_queue_input(&session()).is_none());
    let panel = client.queue_panel(&session()).expect("panel");
    assert!(panel
        .input_error()
        .expect("input failure")
        .contains("stopped advancing"));
    assert!(panel.read_error().is_none());
    assert!(
        client.queue_is_current(&session()),
        "preview remains usable"
    );
}

#[test]
fn input_assembly_rejects_a_rounded_utf8_start_that_would_duplicate_already_read_bytes() {
    let (mut client, connection, read) = reading(4);
    let first = text_reply(&mut client, single_read(&read), "aa", false);
    let outgoing = single_read(&first);
    let Command::QueueText {
        expected,
        entry,
        offset,
        ..
    } = command(outgoing).command
    else {
        panic!("second input chunk")
    };
    assert_eq!(offset.get(), 2);
    let result = client
        .receive(
            connection,
            Incoming::Success {
                id: outgoing.id(),
                result: json!({"session":session(),"revision":expected,"entry":entry,
            "offset":"1","next_offset":"4","complete":true,"text":"€"}),
            },
        )
        .expect("valid generic rounded UTF-8 range");
    assert!(result.outgoing.is_empty());
    assert!(
        client.take_queue_input(&session()).is_none(),
        "overlap cannot become editable text"
    );
    assert!(client
        .queue_panel(&session())
        .expect("panel")
        .input_error()
        .is_some());
    assert!(
        client.queue_is_current(&session()),
        "the checked preview remains usable"
    );
}

#[test]
fn changed_revisions_and_cancelled_reads_cannot_publish_old_input_or_restore_it_after_eof() {
    let (mut client, connection, read) = reading(3);
    let changed = notify(
        &mut client,
        connection,
        Notification::SessionEvent(event(
            11,
            Event::QueueChanged {
                queue: Some(page(0, 1, 11, 'b').queue),
            },
        )),
    );
    assert_eq!(changed.outgoing.len(), 1, "replacement previews first");
    text_reply(&mut client, single_read(&read), "old", true);
    assert!(client.take_queue_input(&session()).is_none());
    assert!(client
        .queue_panel(&session())
        .expect("panel")
        .input_error()
        .is_some());
    reply(
        &mut client,
        single_read(&changed),
        json!(page(0, 1, 11, 'b')),
    );
    let pending = client
        .begin_queue_input(&session())
        .expect("new selected input");
    client.cancel_queue_input(&session());
    let late = text_reply(&mut client, single_read(&pending), "new", true);
    assert!(late.replies.is_empty());
    assert!(client.take_queue_input(&session()).is_none());
    client
        .begin_queue_input(&session())
        .expect("read before EOF");
    client.disconnect(connection);
    assert!(!client
        .queue_panel(&session())
        .expect("retained panel")
        .is_reading_input());
    assert!(client.take_queue_input(&session()).is_none());
}

#[test]
fn attachment_inputs_and_native_ceiling_fail_before_any_text_request_is_allocated() {
    for bytes in [None, Some(16 * 1024 * 1024 + 1)] {
        let (mut client, _) = attached();
        let opened = client
            .open_queue_panel(&session(), None, QueueReadMode::Refresh)
            .expect("open");
        let mut preview = page(0, 1, 10, 'a');
        preview.entries[0].text_bytes = bytes.map(ByteLength::new);
        reply(&mut client, single_read(&opened), json!(preview));
        assert!(matches!(
            client.begin_queue_input(&session()),
            Err(ClientError::Capacity(_) | ClientError::InvalidPayload(_))
        ));
        assert!(client
            .queue_panel(&session())
            .expect("panel")
            .input_error()
            .is_none());
        assert!(client.operations().iter().next().is_none());
    }
}

#[test]
fn a_native_input_failure_stops_reading_until_the_user_explicitly_retries() {
    let (mut client, connection, read) = reading(3);
    let failed = client
        .receive(
            connection,
            Incoming::Failure {
                id: Some(single_read(&read).id()),
                error: RpcError {
                    code: -32001,
                    message: "selected input disappeared".into(),
                },
            },
        )
        .expect("failure");
    assert!(failed.outgoing.is_empty());
    assert!(failed.failures.is_empty());
    assert_eq!(
        client.queue_panel(&session()).expect("panel").input_error(),
        Some("selected input disappeared")
    );
    let retry = client
        .begin_queue_input(&session())
        .expect("explicit retry");
    text_reply(&mut client, single_read(&retry), "row", true);
    assert_eq!(
        client
            .take_queue_input(&session())
            .expect("retried input")
            .text,
        "row"
    );
}

fn reading(bytes: u64) -> (Client, ConnectionId, Effects) {
    let (mut client, connection) = attached();
    let opened = client
        .open_queue_panel(&session(), None, QueueReadMode::Refresh)
        .expect("open");
    let mut preview = page(0, 1, 10, 'a');
    preview.entries[0].text_bytes = Some(ByteLength::new(bytes));
    reply(&mut client, single_read(&opened), json!(preview));
    let read = client.begin_queue_input(&session()).expect("read input");
    (client, connection, read)
}

fn text_reply(client: &mut Client, outgoing: &Outbound, text: &str, complete: bool) -> Effects {
    let Command::QueueText {
        session,
        expected,
        entry,
        offset,
        ..
    } = command(outgoing).command
    else {
        panic!("selected input chunk")
    };
    reply(
        client,
        outgoing,
        json!({"session":session,"revision":expected,"entry":entry,
        "offset":offset,"next_offset":(offset.get() + text.len() as u64).to_string(),
        "complete":complete,"text":text}),
    )
}
