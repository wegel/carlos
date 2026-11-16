//! Exercise complete shared model lists, future facts and retired readers without a terminal.

use carlos_proto::commands::{Command, RpcError};
use carlos_proto::events::{Event, Notification};
use carlos_proto::ids::Sequence;
use carlos_proto::settings::{ModelInfo, SettingsPage};
use serde_json::json;

use super::client::{attached, command, greet, notify, reply};
use super::queue_panel::single_read;
use super::settings::facts;
use super::{entry_id, event, session, snapshot};
use crate::codec::Incoming;
use crate::{Client, ConnectionId, Effects, Outbound, ReadReply};

#[test]
fn a_complete_model_list_waits_for_its_numbered_facts_and_transfers_without_a_second_cache() {
    let (mut client, connection) = attached();
    let opened = client.open_settings_panel(&session()).expect("open models");
    let held = reply(
        &mut client,
        single_read(&opened),
        json!(page(0, 65, 11, 'a', 8)),
    );
    assert!(held.replies.is_empty());
    assert!(held.outgoing.is_empty());
    assert!(client
        .settings_panel(&session())
        .expect("panel")
        .catalog()
        .is_none());
    assert!(client
        .session(&session())
        .expect("facts")
        .settings()
        .is_none());
    let mut effects = notify(
        &mut client,
        connection,
        Notification::SessionEvent(event(
            11,
            Event::SettingsChanged {
                settings: Some(page(0, 65, 11, 'a', 8).settings),
            },
        )),
    );
    for offset in [32, 64] {
        let outgoing = single_read(&effects);
        let Command::SettingsRead {
            offset: requested,
            expected,
            ..
        } = command(outgoing).command
        else {
            panic!("next model page")
        };
        assert_eq!(requested, offset);
        assert_eq!(expected, Some(facts('a').revision));
        effects = reply(&mut client, outgoing, json!(page(offset, 65, 11, 'a', 8)));
    }
    assert!(effects.outgoing.is_empty());
    let complete = client
        .take_settings_catalog(&session())
        .expect("all checked models");
    assert_eq!(complete.models.len(), 65);
    assert_eq!(complete.models[64].model, "model-64");
    assert_eq!(complete.facts.revision, facts('a').revision);
    assert!(client.take_settings_catalog(&session()).is_none());
    assert!(!client
        .settings_panel(&session())
        .expect("panel")
        .is_reading());
    assert!(client.operations().iter().next().is_none());
}

#[test]
fn a_failed_old_continuation_restarts_from_the_new_revision_without_mixing_model_lists() {
    let (mut client, connection) = with_settings(65);
    let opened = client.open_settings_panel(&session()).expect("open");
    let pending = reply(
        &mut client,
        single_read(&opened),
        json!(page(0, 65, 10, 'a', 8)),
    );
    let changed = notify(
        &mut client,
        connection,
        Notification::SessionEvent(event(
            11,
            Event::SettingsChanged {
                settings: Some(page(0, 1, 11, 'b', 8).settings),
            },
        )),
    );
    assert!(changed.outgoing.is_empty());
    let next = fail(
        &mut client,
        single_read(&pending),
        "old model revision retired",
    );
    assert!(next.failures.is_empty());
    let Command::SettingsRead {
        offset, expected, ..
    } = command(single_read(&next)).command
    else {
        panic!("new first page")
    };
    assert_eq!(offset, 0);
    assert!(expected.is_none());
    let _ = reply(
        &mut client,
        single_read(&next),
        json!(page(0, 1, 11, 'b', 8)),
    );
    let complete = client
        .take_settings_catalog(&session())
        .expect("new complete list");
    assert_eq!(complete.models.len(), 1);
    assert_eq!(complete.facts.revision, facts('b').revision);
    assert!(client
        .settings_panel(&session())
        .expect("panel")
        .read_error()
        .is_none());
}

#[test]
fn explicit_reads_remain_usable_and_closed_read_failures_cannot_replace_a_reopened_reader() {
    let (mut client, connection) = with_settings(1);
    let old = client.open_settings_panel(&session()).expect("old reader");
    let _ = client.close_settings_panel(&session());
    let fresh = client
        .open_settings_panel(&session())
        .expect("fresh reader");
    let late = fail(
        &mut client,
        single_read(&old),
        "obsolete private read failure",
    );
    assert!(late.failures.is_empty());
    assert!(client
        .settings_panel(&session())
        .expect("fresh model")
        .is_reading());
    let direct = client
        .read(
            connection,
            Command::SettingsRead {
                session: session(),
                expected: None,
                offset: 0,
                limit: Some(32),
            },
        )
        .expect("unowned direct read");
    let unowned = reply(&mut client, &direct, json!(page(0, 1, 10, 'a', 8)));
    assert!(matches!(unowned.replies[0].reply, ReadReply::Settings(_)));
    let _ = reply(
        &mut client,
        single_read(&fresh),
        json!(page(0, 1, 10, 'a', 8)),
    );
    client.disconnect(connection);
    assert!(client.close_settings_panel(&session()).outgoing.is_empty());
    let (_, hello) = greet(&mut client, true, "d");
    let restored = reply(
        &mut client,
        single_read(&hello),
        json!({"kind":"replay","seq":"10","events":[]}),
    );
    assert!(restored.outgoing.is_empty());
    assert!(client.settings_panel(&session()).is_none());
}

#[test]
fn oversized_lists_stop_with_an_explicit_error_instead_of_exposing_a_truncated_catalog() {
    let (mut client, connection) = with_settings(2200);
    let mut effects = client
        .open_settings_panel(&session())
        .expect("open large catalog");
    for offset in (0..2200).step_by(32) {
        effects = reply(
            &mut client,
            single_read(&effects),
            json!(page(offset, 2200, 10, 'a', 4096)),
        );
        if effects.outgoing.is_empty() {
            break;
        }
    }
    let panel = client
        .settings_panel(&session())
        .expect("failed reader remains explicit");
    assert!(!panel.is_reading());
    assert!(panel
        .read_error()
        .expect("budget error")
        .contains("eight MiB"));
    assert!(client.take_settings_catalog(&session()).is_none());
    let unrelated = notify(
        &mut client,
        connection,
        Notification::SessionEvent(event(
            11,
            Event::TextAppended {
                entry: entry_id(10),
                text: "unrelated delta".into(),
            },
        )),
    );
    assert!(
        unrelated.outgoing.is_empty(),
        "no automatic fetch loop after budget refusal"
    );
}

#[test]
fn reconnect_restarts_an_incomplete_list_but_keeps_an_already_transferred_editor_independent() {
    let (mut client, connection) = with_settings(33);
    let opened = client
        .open_settings_panel(&session())
        .expect("initial reader");
    let _ = reply(
        &mut client,
        single_read(&opened),
        json!(page(0, 33, 10, 'a', 8)),
    );
    client.disconnect(connection);
    let (_, hello) = greet(&mut client, true, "d");
    let initial = reply(
        &mut client,
        single_read(&hello),
        json!({"kind":"replay","seq":"10","events":[]}),
    );
    let Command::SettingsRead {
        offset, expected, ..
    } = command(single_read(&initial)).command
    else {
        panic!("restart after current subscription")
    };
    assert_eq!(offset, 0);
    assert!(expected.is_none());
    let next = reply(
        &mut client,
        single_read(&initial),
        json!(page(0, 33, 10, 'a', 8)),
    );
    let _ = reply(
        &mut client,
        single_read(&next),
        json!(page(32, 33, 10, 'a', 8)),
    );
    let mut editor = client
        .take_settings_catalog(&session())
        .expect("complete private baseline");
    editor.models[0].display_name = "private editor choice".into();
    client.disconnect(single_read(&initial).connection());
    let (_, hello) = greet(&mut client, true, "d");
    let resumed = reply(
        &mut client,
        single_read(&hello),
        json!({"kind":"replay","seq":"10","events":[]}),
    );
    assert!(
        resumed.outgoing.is_empty(),
        "a transferred list is a frozen editor, not a reconnect refetch"
    );
    assert_eq!(editor.models[0].display_name, "private editor choice");
}

fn with_settings(count: u32) -> (Client, ConnectionId) {
    let (mut client, connection) = attached();
    let mut state = snapshot();
    state.settings = Some(page(0, count, 10, 'a', 8).settings);
    let _ = notify(
        &mut client,
        connection,
        Notification::Snapshot {
            snapshot: Box::new(state),
        },
    );
    (client, connection)
}

fn fail(client: &mut Client, outgoing: &Outbound, message: &str) -> Effects {
    client
        .receive(
            outgoing.connection(),
            Incoming::Failure {
                id: Some(outgoing.id()),
                error: RpcError {
                    code: -32001,
                    message: message.into(),
                },
            },
        )
        .expect("checked failure")
}

fn page(offset: u32, count: u32, seq: u64, version: char, label_bytes: usize) -> SettingsPage {
    let mut settings = facts(version);
    settings.model_count = count;
    let end = (offset + 32).min(count);
    SettingsPage {
        session: session(),
        seq: Sequence::new(seq),
        settings,
        offset,
        models: (offset..end)
            .map(|index| ModelInfo {
                model: format!("model-{index}"),
                display_name: "x".repeat(label_bytes),
                supported_efforts: vec!["low".into(), "high".into()],
                default_effort: Some("low".into()),
                is_default: index == 0,
            })
            .collect(),
        next: (end < count).then_some(end),
    }
}
