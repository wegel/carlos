//! Keep bounded history windows contiguous and reject malformed replacement pages atomically.

use carlos_proto::events::Event;
use carlos_proto::ids::Sequence;
use carlos_proto::session::History;

use super::{entry, entry_id, event, snapshot};
use crate::{ClientError, Replica};

#[test]
fn older_available_snapshots_still_enforce_the_client_byte_budget() {
    let mut initial = snapshot();
    let text = "x".repeat(4096);
    initial.entries = (0..8000).map(|id| entry(id, &text)).collect();
    let mut replica = Replica::new(initial).expect("bounded snapshot");
    assert!(replica.entry(&entry_id(0)).is_none());
    assert!(replica.entry(&entry_id(7999)).is_some());
    assert!(replica.has_older());
    let anchor = replica.entries().next().expect("front").id.clone();
    let page = History {
        entries: vec![entry(0, &text)],
        has_older: false,
        seq: replica.sequence(),
    };
    let changed = replica.merge_history(&anchor, page).expect("older window");
    assert_eq!(changed.removed_back, 1);
    assert!(replica.has_newer());
    replica
        .apply(event(
            11,
            Event::EntryAdded {
                entry: entry(8000, "live"),
            },
        ))
        .expect("live event without a gap in the older window");
    assert!(replica.entry(&entry_id(8000)).is_none());
    let mut latest = snapshot();
    latest.seq = Sequence::new(11);
    latest.entries = vec![entry(8000, "live")];
    replica.replace(latest).expect("return to latest");
    assert!(!replica.has_newer());
}

#[test]
fn duplicate_pages_and_snapshots_do_not_partially_replace_valid_facts() {
    let mut replica = Replica::new(snapshot()).expect("snapshot");
    let page = History {
        entries: vec![entry(8, "first"), entry(8, "duplicate")],
        has_older: false,
        seq: replica.sequence(),
    };
    assert!(matches!(
        replica.merge_history(&entry_id(10), page),
        Err(ClientError::DuplicateEntry(_))
    ));
    assert!(replica.entry(&entry_id(8)).is_none());
    assert_eq!(
        replica.entry(&entry_id(10)).expect("retained").text,
        "hello"
    );
    let mut invalid = snapshot();
    invalid.seq = Sequence::new(11);
    invalid.entries.push(entry(10, "duplicate"));
    assert!(replica.replace(invalid).is_err());
    assert_eq!(replica.sequence(), Sequence::new(10));
    assert_eq!(replica.entries().count(), 1);
}

#[test]
fn exhausting_unseen_changes_advances_the_history_fence_without_an_unbounded_log() {
    let mut replica = Replica::new(snapshot()).expect("snapshot");
    for index in 0..1025 {
        replica
            .apply(event(
                index + 11,
                Event::TextAppended {
                    entry: entry_id(index + 100),
                    text: "new".into(),
                },
            ))
            .expect("unseen row change");
    }
    let old_page = History {
        entries: vec![entry(1, "unrelated")],
        has_older: false,
        seq: Sequence::new(10),
    };
    assert_eq!(
        replica
            .merge_history(&entry_id(10), old_page)
            .expect_err("retired fence"),
        ClientError::StaleHistory
    );
    let fresh_page = History {
        entries: vec![entry(1, "unrelated")],
        has_older: false,
        seq: replica.sequence(),
    };
    replica
        .merge_history(&entry_id(10), fresh_page)
        .expect("current history");
}
