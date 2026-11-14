//! Keep queue facts numbered while selections, submitted text and editors stay outside them.

use carlos_proto::events::{Event, Notification};
use carlos_proto::ids::Sequence;
use carlos_proto::queue::{QueueFacts, QueuePage};

use super::client::{attached, notify};
use super::{entry_id, event, session, snapshot};
use crate::{ClientError, Replica};

#[test]
fn queue_events_replace_only_shared_facts_and_duplicate_delivery_has_no_effect() {
    let mut replica = Replica::new(snapshot()).expect("snapshot");
    let facts = QueueFacts {
        revision: "a".repeat(64).parse().expect("queue revision"),
        known: true,
        reading: false,
        pending: true,
        count: 3,
        read_error: None,
        notice: "Native change pending".into(),
    };
    let update = event(
        11,
        Event::QueueChanged {
            queue: Some(facts.clone()),
        },
    );
    assert!(
        replica
            .apply(update.clone())
            .expect("new queue facts")
            .queue
    );
    assert_eq!(replica.queue(), Some(&facts));
    assert!(!replica.apply(update).expect("duplicate").queue);
    let mut reset = snapshot();
    reset.seq = Sequence::new(13);
    replica.replace(reset).expect("replace snapshot");
    assert!(replica.queue().is_none());
}

fn queue_page(seq: u64, revision: &str) -> QueuePage {
    QueuePage {
        session: session(),
        seq: Sequence::new(seq),
        queue: QueueFacts {
            revision: revision.repeat(64).parse().expect("queue revision"),
            known: true,
            reading: false,
            pending: false,
            count: 0,
            read_error: None,
            notice: String::new(),
        },
        entries: Vec::new(),
        has_more: false,
    }
}

#[test]
fn queue_reads_keep_unrelated_text_but_reject_late_future_and_replaced_headers() {
    let mut replica = Replica::new(snapshot()).expect("session");
    replica
        .merge_queue(&queue_page(10, "a"))
        .expect("current queue");
    replica
        .apply(event(
            11,
            Event::TextAppended {
                entry: entry_id(10),
                text: " later".into(),
            },
        ))
        .expect("unrelated fact");
    assert!(
        !replica
            .merge_queue(&queue_page(10, "a"))
            .expect("still current queue")
            .queue
    );
    assert!(matches!(
        replica.merge_queue(&queue_page(12, "b")),
        Err(ClientError::StaleQueue)
    ));
    replica
        .apply(event(
            12,
            Event::QueueChanged {
                queue: Some(queue_page(12, "b").queue),
            },
        ))
        .expect("new queue");
    assert!(matches!(
        replica.merge_queue(&queue_page(11, "a")),
        Err(ClientError::StaleQueue)
    ));
    assert_eq!(replica.queue(), Some(&queue_page(12, "b").queue));
    let mut reset = snapshot();
    reset.seq = Sequence::new(14);
    reset.queue = Some(queue_page(14, "c").queue);
    replica.replace(reset).expect("replaced snapshot");
    assert!(matches!(
        replica.merge_queue(&queue_page(12, "b")),
        Err(ClientError::StaleQueue)
    ));
    assert_eq!(replica.queue(), Some(&queue_page(14, "c").queue));
}

#[test]
fn client_merges_queue_reads_into_its_single_numbered_fact_copy() {
    let (mut client, connection) = attached();
    let changes = client
        .merge_queue(&queue_page(10, "a"))
        .expect("queue read");
    assert_eq!(changes.sessions.len(), 1);
    assert!(changes.sessions[0].changes.queue);
    assert_eq!(
        client.session(&session()).expect("session").queue(),
        Some(&queue_page(10, "a").queue)
    );
    notify(
        &mut client,
        connection,
        Notification::SessionEvent(event(
            11,
            Event::QueueChanged {
                queue: Some(queue_page(11, "b").queue),
            },
        )),
    );
    assert!(matches!(
        client.merge_queue(&queue_page(10, "a")),
        Err(ClientError::StaleQueue)
    ));
    assert!(matches!(
        client.merge_queue(&queue_page(12, "c")),
        Err(ClientError::StaleQueue)
    ));
    assert_eq!(
        client.session(&session()).expect("current").queue(),
        Some(&queue_page(11, "b").queue)
    );
}
