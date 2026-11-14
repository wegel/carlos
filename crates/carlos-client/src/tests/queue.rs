//! Keep queue facts numbered while selections, submitted text and editors stay outside them.

use carlos_proto::events::Event;
use carlos_proto::queue::QueueFacts;

use super::{event, snapshot};
use crate::Replica;

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
    reset.seq = carlos_proto::ids::Sequence::new(13);
    replica.replace(reset).expect("replace snapshot");
    assert!(replica.queue().is_none());
}
