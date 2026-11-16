//! Fence task headers and private pages against one authoritative numbered session copy.

use carlos_proto::events::{Event, Notification};
use carlos_proto::ids::Sequence;
use carlos_proto::task_facts::{TaskCounts, TaskFacts, TaskSnapshot};
use carlos_proto::tasks::TaskPage;

use super::client::{attached, notify};
use super::{entry_id, event, session, snapshot};
use crate::{Client, ClientError};

pub(super) fn facts(version: &str) -> TaskFacts {
    TaskFacts {
        revision: version.repeat(64).parse().expect("task revision"),
        overview_revision: version.repeat(64).parse().expect("overview revision"),
        known: true,
        reading: false,
        closed: false,
        counts: TaskCounts::default(),
        native_older_available: false,
        read_error: None,
        notice: String::new(),
    }
}

fn header(seq: u64, version: &str) -> TaskSnapshot {
    TaskSnapshot {
        session: session(),
        seq: Sequence::new(seq),
        tasks: facts(version),
    }
}

fn page(seq: u64, version: &str) -> TaskPage {
    TaskPage {
        session: session(),
        seq: Sequence::new(seq),
        revision: facts(version).revision,
        known: true,
        reading: false,
        closed: false,
        tasks: Vec::new(),
        has_more: false,
        native_older_available: false,
        read_error: None,
    }
}

fn task_sequence(client: &Client) -> Sequence {
    client
        .session(&session())
        .expect("task facts")
        .task_sequence()
}

#[test]
fn current_task_headers_merge_once_and_private_pages_never_replace_shared_facts() {
    let (mut client, connection) = attached();
    let effects = client
        .merge_tasks(&header(10, "a"))
        .expect("current task header");
    assert_eq!(effects.sessions.len(), 1);
    assert!(effects.sessions[0].changes.tasks);
    let info = client.session(&session()).expect("session").info().clone();
    let _ = notify(
        &mut client,
        connection,
        Notification::SessionEvent(event(11, Event::InfoChanged { info })),
    );
    assert_eq!(task_sequence(&client), Sequence::new(10));
    let _ = notify(
        &mut client,
        connection,
        Notification::SessionEvent(event(
            12,
            Event::TextAppended {
                entry: entry_id(10),
                text: "root-only delta".into(),
            },
        )),
    );
    assert_eq!(task_sequence(&client), Sequence::new(10));
    client
        .check_task_page(&page(10, "a"))
        .expect("unrelated newer session facts");
    assert_eq!(
        client.session(&session()).expect("one copy").tasks(),
        Some(&facts("a"))
    );
    assert!(matches!(
        client.check_task_page(&page(12, "b")),
        Err(ClientError::StaleTasks)
    ));
    assert_eq!(
        client
            .session(&session())
            .expect("no future overwrite")
            .tasks(),
        Some(&facts("a"))
    );
}

#[test]
fn late_future_and_reset_task_headers_cannot_overwrite_the_accepted_copy() {
    let (mut client, connection) = attached();
    let _ = client
        .merge_tasks(&header(10, "a"))
        .expect("initial header");
    assert!(matches!(
        client.merge_tasks(&header(11, "b")),
        Err(ClientError::StaleTasks)
    ));
    let _ = notify(
        &mut client,
        connection,
        Notification::SessionEvent(event(
            11,
            Event::TasksChanged {
                tasks: Some(facts("b")),
            },
        )),
    );
    assert_eq!(task_sequence(&client), Sequence::new(11));
    assert!(matches!(
        client.merge_tasks(&header(10, "a")),
        Err(ClientError::StaleTasks)
    ));
    assert!(matches!(
        client.check_task_page(&page(10, "a")),
        Err(ClientError::StaleTasks)
    ));
    let mut reset = snapshot();
    reset.seq = Sequence::new(12);
    reset.tasks = Some(facts("c"));
    let _ = notify(
        &mut client,
        connection,
        Notification::Snapshot {
            snapshot: Box::new(reset),
        },
    );
    assert!(matches!(
        client.merge_tasks(&header(11, "b")),
        Err(ClientError::StaleTasks)
    ));
    assert_eq!(task_sequence(&client), Sequence::new(12));
    assert_eq!(
        client.session(&session()).expect("reset facts").tasks(),
        Some(&facts("c"))
    );
}
