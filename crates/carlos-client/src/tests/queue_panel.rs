//! Drive a device's shared queue reader without a terminal, timer or second fact copy.

use carlos_proto::commands::{Command, RpcError};
use carlos_proto::events::{Event, Notification};
use carlos_proto::ids::{QueueEntryId, QueueRevision, Sequence};
use carlos_proto::queue::{QueuePage, MAX_PAGE_ENTRIES};
use serde_json::json;

use super::client::{attached, command, greet, notify, reply};
use super::{entry_id, event, session};
use crate::codec::Incoming;
use crate::panels::PanelKind;
use crate::panels::QueueReadMode;
use crate::{Client, Effects, Outbound, ReadReply};

#[test]
fn queue_reader_walks_all_supported_rows_and_retains_only_the_selected_page() {
    let (mut client, _) = attached();
    let mut effects = client
        .open_queue_panel(&session(), Some(id(4095)), QueueReadMode::Refresh)
        .expect("open");
    for start in (0usize..4096).step_by(MAX_PAGE_ENTRIES) {
        let outgoing = single_read(&effects);
        let Command::QueueRead {
            expected,
            after,
            refresh,
            ..
        } = command(outgoing).command
        else {
            panic!("queue page request")
        };
        assert_eq!(after, start.checked_sub(1).map(id));
        assert_eq!(refresh, start == 0);
        assert_eq!(expected, (start != 0).then(|| revision('a')));
        effects = reply(&mut client, outgoing, json!(page(start, 4096, 10, 'a')));
        assert!(
            effects.replies.is_empty(),
            "owned read stays in the shared model"
        );
        assert!(
            client
                .queue_panel(&session())
                .expect("panel")
                .page()
                .entries()
                .len()
                <= 32
        );
    }
    assert!(effects.outgoing.is_empty());
    assert!(client.queue_is_current(&session()));
    let panel = client.queue_panel(&session()).expect("panel");
    assert_eq!(panel.page().position(), Some(4095));
    assert_eq!(panel.selection(), Some(id(4095).as_str()));
    assert_eq!(panel.page().entries().len(), 32);
    let previous = client
        .move_queue_panel(&session(), -1)
        .expect("explicit move");
    assert!(
        previous.outgoing.is_empty(),
        "the retained page already proves this row"
    );
    assert_eq!(
        client
            .queue_panel(&session())
            .expect("panel")
            .page()
            .position(),
        Some(4094)
    );
}

#[test]
fn a_future_page_waits_for_numbered_facts_even_when_the_last_event_changes_no_queue_data() {
    let (mut client, connection) = attached();
    let effects = client
        .open_queue_panel(&session(), None, QueueReadMode::Refresh)
        .expect("open");
    let future = reply(
        &mut client,
        single_read(&effects),
        json!(page(0, 1, 11, 'a')),
    );
    assert!(future.outgoing.is_empty());
    assert!(!client.queue_is_current(&session()));
    assert!(client.session(&session()).expect("facts").queue().is_none());
    let settled = notify(
        &mut client,
        connection,
        Notification::SessionEvent(event(
            11,
            Event::TextAppended {
                entry: entry_id(10),
                text: "".into(),
            },
        )),
    );
    assert!(settled.outgoing.is_empty());
    assert!(settled
        .panels
        .iter()
        .any(|change| change.kind == PanelKind::Queue));
    assert!(client.queue_is_current(&session()));
    assert_eq!(
        client.session(&session()).expect("facts").queue(),
        Some(&page(0, 1, 11, 'a').queue)
    );
}

#[test]
fn cached_queue_interest_keeps_known_facts_usable_without_forcing_native_invalidation() {
    let (mut client, _) = attached();
    let opened = client
        .open_queue_panel(&session(), None, QueueReadMode::Refresh)
        .expect("sheet refresh");
    let _ = reply(
        &mut client,
        single_read(&opened),
        json!(page(0, 1, 10, 'a')),
    );
    let _ = client.close_queue_panel(&session());
    let watched = client
        .open_queue_panel(&session(), None, QueueReadMode::Cached)
        .expect("queue mode");
    let Command::QueueRead {
        refresh, expected, ..
    } = command(single_read(&watched)).command
    else {
        panic!("bounded cached previews")
    };
    assert!(
        !refresh,
        "changing the composer policy must not invalidate native facts"
    );
    assert_eq!(expected, Some(revision('a')));
    assert!(
        client
            .session(&session())
            .expect("accepted facts")
            .queue()
            .expect("known queue")
            .known
    );
}

#[test]
fn a_late_failure_cannot_replace_a_new_revision_or_select_a_different_native_entry() {
    let (mut client, connection) = attached();
    let opened = client
        .open_queue_panel(&session(), Some(id(64)), QueueReadMode::Refresh)
        .expect("open");
    let first = reply(
        &mut client,
        single_read(&opened),
        json!(page(0, 65, 10, 'a')),
    );
    let replaced = notify(
        &mut client,
        connection,
        Notification::SessionEvent(event(
            11,
            Event::QueueChanged {
                queue: Some(page(0, 1, 11, 'b').queue),
            },
        )),
    );
    assert!(
        replaced.outgoing.is_empty(),
        "one owned read remains in flight"
    );
    let failed = fail(
        &mut client,
        connection,
        single_read(&first),
        "old revision retired",
    );
    assert!(failed.failures.is_empty());
    assert!(client
        .queue_panel(&session())
        .expect("panel")
        .read_error()
        .is_none());
    let Command::QueueRead {
        expected, after, ..
    } = command(single_read(&failed)).command
    else {
        panic!("replacement page")
    };
    assert_eq!(expected, Some(revision('b')));
    assert!(after.is_none());
    let settled = reply(
        &mut client,
        single_read(&failed),
        json!(page(0, 1, 11, 'b')),
    );
    assert!(settled.outgoing.is_empty());
    let panel = client.queue_panel(&session()).expect("panel");
    assert_eq!(panel.selection(), Some(id(64).as_str()));
    assert!(panel.page().selected(panel.selection()).is_none());
}

#[test]
fn closing_offline_never_restores_the_panel_and_reopening_cannot_accept_old_replies() {
    let (mut client, connection) = attached();
    let old = client
        .open_queue_panel(&session(), None, QueueReadMode::Refresh)
        .expect("open");
    let _ = client.close_queue_panel(&session());
    let reopened = client
        .open_queue_panel(&session(), None, QueueReadMode::Refresh)
        .expect("reopen");
    let late = reply(&mut client, single_read(&old), json!(page(0, 1, 10, 'a')));
    assert!(late.replies.is_empty());
    assert!(client
        .queue_panel(&session())
        .expect("new panel")
        .page()
        .entries()
        .is_empty());
    let _ = reply(
        &mut client,
        single_read(&reopened),
        json!(page(0, 1, 10, 'b')),
    );
    client.disconnect(connection);
    assert!(!client.queue_is_current(&session()));
    let closed = client.close_queue_panel(&session());
    assert!(closed.outgoing.is_empty());
    let (_, resumed) = greet(&mut client, true, "d");
    let restored = reply(
        &mut client,
        single_read(&resumed),
        json!({"kind":"replay","events":[],"seq":"10"}),
    );
    assert!(restored.outgoing.is_empty());
    assert!(client.queue_panel(&session()).is_none());
}

#[test]
fn an_unfinished_explicit_refresh_restarts_only_after_current_subscription_acceptance() {
    let (mut client, connection) = attached();
    let _ = client
        .open_queue_panel(&session(), None, QueueReadMode::Refresh)
        .expect("open before EOF");
    client.disconnect(connection);
    let (new_connection, resumed) = greet(&mut client, true, "d");
    assert!(!client.queue_is_current(&session()));
    let restored = reply(
        &mut client,
        single_read(&resumed),
        json!({"kind":"replay","events":[],"seq":"10"}),
    );
    let Command::QueueRead {
        refresh, expected, ..
    } = command(single_read(&restored)).command
    else {
        panic!("restored explicit refresh")
    };
    assert!(refresh);
    assert!(expected.is_none());
    assert!(client.session_is_ready(new_connection, &session()));
    let _ = reply(
        &mut client,
        single_read(&restored),
        json!(page(0, 1, 10, 'a')),
    );
    assert!(client.queue_is_current(&session()));
}

#[test]
fn explicit_reads_remain_unowned_and_panel_failures_wait_for_explicit_retry() {
    let (mut client, connection) = attached();
    let opened = client
        .open_queue_panel(&session(), None, QueueReadMode::Refresh)
        .expect("open");
    let direct = client
        .read(
            connection,
            Command::QueueRead {
                session: session(),
                expected: None,
                after: None,
                limit: Some(32),
                refresh: false,
            },
        )
        .expect("independent read");
    let unowned = reply(&mut client, &direct, json!(page(0, 1, 10, 'a')));
    assert!(matches!(unowned.replies[0].reply, ReadReply::Queue(_)));
    let failed = fail(
        &mut client,
        connection,
        single_read(&opened),
        "native reader failed",
    );
    assert!(failed.outgoing.is_empty());
    assert_eq!(
        client.queue_panel(&session()).expect("panel").read_error(),
        Some("native reader failed")
    );
    let unrelated = notify(
        &mut client,
        connection,
        Notification::SessionEvent(event(
            11,
            Event::TextAppended {
                entry: entry_id(10),
                text: "ordinary token".into(),
            },
        )),
    );
    assert!(unrelated.outgoing.is_empty());
    let retried = client
        .refresh_queue_panel(&session())
        .expect("explicit retry");
    assert!(matches!(
        command(single_read(&retried)).command,
        Command::QueueRead { refresh: true, .. }
    ));
    assert!(
        client.operations().iter().next().is_none(),
        "reads never allocate native work"
    );
}

pub(super) fn single_read(effects: &Effects) -> &Outbound {
    assert_eq!(effects.outgoing.len(), 1, "exactly one next read");
    &effects.outgoing[0]
}

fn fail(
    client: &mut Client,
    connection: crate::ConnectionId,
    outgoing: &Outbound,
    message: &str,
) -> Effects {
    client
        .receive(
            connection,
            Incoming::Failure {
                id: Some(outgoing.id()),
                error: RpcError {
                    code: -32001,
                    message: message.into(),
                },
            },
        )
        .expect("read failure")
}

pub(super) fn id(index: usize) -> QueueEntryId {
    format!("{index:064x}").parse().expect("queue identity")
}

fn revision(letter: char) -> QueueRevision {
    letter.to_string().repeat(64).parse().expect("revision")
}

pub(super) fn page(start: usize, count: usize, seq: u64, rev: char) -> QueuePage {
    let end = (start + MAX_PAGE_ENTRIES).min(count);
    serde_json::from_value(json!({
        "session":session(), "seq":Sequence::new(seq), "queue":{
            "revision":revision(rev), "known":true,"reading":false,"pending":false,
            "count":count,"read_error":null,"notice":""
        }, "entries":(start..end).map(|index| json!({
            "id":id(index),"client_message":"human","preview":"row","preview_complete":true,"text_bytes":"3"
        })).collect::<Vec<_>>(), "has_more":end<count
    })).expect("queue page")
}
