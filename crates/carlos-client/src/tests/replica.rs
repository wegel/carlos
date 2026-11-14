//! Preserve sequence order and reject history that a later unseen row change made stale.

use carlos_proto::events::Event;
use carlos_proto::ids::Sequence;
use carlos_proto::session::History;

use super::{entry, entry_id, event, snapshot};
use crate::{ClientError, Replica};

#[test]
fn a_gap_blocks_later_deltas_until_a_snapshot_and_duplicate_delivery_never_appends_twice() {
    let mut replica = Replica::new(snapshot()).expect("snapshot");
    let append = event(
        11,
        Event::TextAppended {
            entry: entry_id(10),
            text: " world".into(),
        },
    );
    replica.apply(append.clone()).expect("append");
    replica.apply(append).expect("duplicate ignored");
    assert_eq!(
        replica.entry(&entry_id(10)).expect("row").text,
        "hello world"
    );
    assert!(matches!(
        replica.apply(event(
            13,
            Event::TextAppended {
                entry: entry_id(10),
                text: "bad".into()
            }
        )),
        Err(ClientError::SequenceGap { .. })
    ));
    assert!(replica.is_stale());
    assert!(replica
        .apply(event(
            12,
            Event::TextAppended {
                entry: entry_id(10),
                text: "late".into()
            }
        ))
        .is_err());
    let mut replacement = snapshot();
    replacement.seq = Sequence::new(13);
    replica.replace(replacement).expect("replace");
    assert!(!replica.is_stale());
    assert_eq!(replica.entry(&entry_id(10)).expect("new row").text, "hello");
}

#[test]
fn a_newer_change_to_an_unloaded_row_rejects_a_stale_history_page() {
    let mut replica = Replica::new(snapshot()).expect("snapshot");
    replica
        .apply(event(
            11,
            Event::TextAppended {
                entry: entry_id(9),
                text: "new suffix".into(),
            },
        ))
        .expect("unloaded delta");
    let page = History {
        entries: vec![entry(9, "old text")],
        has_older: true,
        seq: Sequence::new(10),
    };
    assert_eq!(
        replica
            .merge_history(&entry_id(10), page)
            .expect_err("newer unseen row"),
        ClientError::StaleHistory
    );
    let page = History {
        entries: vec![entry(9, "old textnew suffix")],
        has_older: true,
        seq: Sequence::new(11),
    };
    replica
        .merge_history(&entry_id(10), page)
        .expect("current page");
    assert_eq!(
        replica.entry(&entry_id(9)).expect("older row").text,
        "old textnew suffix"
    );
}

#[test]
fn unrelated_newer_events_do_not_force_history_reads_to_restart() {
    let mut replica = Replica::new(snapshot()).expect("snapshot");
    replica
        .apply(event(
            11,
            Event::TextAppended {
                entry: entry_id(10),
                text: " new".into(),
            },
        ))
        .expect("loaded delta");
    let page = History {
        entries: vec![entry(9, "older")],
        has_older: false,
        seq: Sequence::new(10),
    };
    replica
        .merge_history(&entry_id(10), page)
        .expect("safe older page");
    assert_eq!(
        replica
            .entries()
            .map(|row| row.text.as_str())
            .collect::<Vec<_>>(),
        ["older", "hello new"]
    );
    assert_eq!(replica.sequence(), Sequence::new(11));
    assert!(!replica.has_older());
}

#[test]
fn a_complete_reset_can_bridge_a_gap_but_a_late_snapshot_cannot_roll_back() {
    let mut replica = Replica::new(snapshot()).expect("snapshot");
    let mut newer = snapshot();
    newer.seq = Sequence::new(15);
    newer.entries[0].text = "replacement".into();
    assert!(
        replica
            .apply(event(15, Event::Reset { snapshot: newer }))
            .expect("complete reset")
            .reset
    );
    replica.replace(snapshot()).expect("ignore late reply");
    assert_eq!(replica.sequence(), Sequence::new(15));
    assert_eq!(
        replica
            .entry(&entry_id(10))
            .expect("retained new facts")
            .text,
        "replacement"
    );
}

#[test]
fn source_format_and_path_changes_redraw_a_row_without_resending_its_text() {
    use carlos_proto::session::{EntryFormat, EntryKind};
    let mut replica = Replica::new(snapshot()).expect("snapshot");
    let facts = serde_json::from_value(serde_json::json!({"kind":"user", "format":"user_steer",
        "file_path":"src/é.rs", "done":true, "tool":null}))
    .expect("facts");
    let changes = replica
        .apply(event(
            11,
            Event::EntryFactsChanged {
                entry: entry_id(10),
                facts,
            },
        ))
        .expect("source facts");
    assert_eq!(changes.entries, [entry_id(10)]);
    let row = replica.entry(&entry_id(10)).expect("row");
    assert_eq!(row.kind, EntryKind::User);
    assert_eq!(row.format, EntryFormat::UserSteer);
    assert_eq!(row.file_path.as_deref(), Some("src/é.rs"));
    assert_eq!(row.text, "hello");
}
