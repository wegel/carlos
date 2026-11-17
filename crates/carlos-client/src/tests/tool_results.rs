//! Fold only proven call/result pairs, keeping repeated equal commands and partial pages intact.

use carlos_proto::events::Event;
use carlos_proto::session::{Entry, EntryKind};
use serde_json::json;

use super::{entry, entry_id, event, snapshot};
use crate::Replica;

fn call(index: u64) -> Entry {
    let mut row = entry(index, "same command");
    row.kind = EntryKind::ToolCall;
    row
}

fn result(index: u64, call: u64) -> Entry {
    serde_json::from_value(json!({"id":entry_id(index),"kind":"tool","text":"",
        "done":true,"operation":null,"tool":{"call":entry_id(call),"kind":"command",
        "label":"same command","label_source":"literal","status":"ok","summary":"26 lines",
        "added":null,"removed":null,"has_output":true,"output_revision":"0"}}))
    .expect("native result")
}

#[test]
fn a_result_folds_only_its_original_call_and_invalidates_that_call() {
    let mut initial = snapshot();
    initial.entries = vec![call(1), call(2)];
    let mut replica = Replica::new(initial).expect("calls");
    let changed = replica
        .apply(event(
            11,
            Event::EntryAdded {
                entry: result(3, 1),
            },
        ))
        .expect("result");
    assert!(replica.has_tool_result(&entry_id(1)));
    assert!(!replica.has_tool_result(&entry_id(2)));
    assert!(changed.entries.contains(&entry_id(1)));
    assert_eq!(
        replica.entry_count(),
        3,
        "folding never deletes original source"
    );
    let mut replacement = result(3, 2);
    replacement.tool.as_mut().expect("tool").summary = Some("27 lines".into());
    let changed = replica
        .apply(event(12, Event::EntryUpdated { entry: replacement }))
        .expect("new facts");
    assert!(!replica.has_tool_result(&entry_id(1)));
    assert!(replica.has_tool_result(&entry_id(2)));
    assert!(changed.entries.contains(&entry_id(1)) && changed.entries.contains(&entry_id(2)));
}

#[test]
fn results_without_a_call_and_ambiguous_pairs_never_hide_another_execution() {
    let mut initial = snapshot();
    initial.entries = vec![call(1), result(3, 1), result(4, 1), result(5, 2)];
    let replica = Replica::new(initial).expect("partial page");
    assert!(
        !replica.has_tool_result(&entry_id(1)),
        "two outcomes must not collapse ambiguously"
    );
    assert!(
        !replica.has_tool_result(&entry_id(2)),
        "call is outside this window"
    );
    assert_eq!(replica.entry_count(), 4);
}

#[test]
fn an_older_call_can_join_its_result_without_mutating_the_original_result() {
    let mut initial = snapshot();
    initial.entries = vec![result(3, 1)];
    let mut replica = Replica::new(initial).expect("result only");
    replica
        .merge_history(
            &entry_id(3),
            carlos_proto::session::History {
                entries: vec![call(1)],
                has_older: false,
                seq: carlos_proto::ids::Sequence::new(10),
            },
        )
        .expect("older call");
    assert!(replica.has_tool_result(&entry_id(1)));
    assert_eq!(replica.entry(&entry_id(3)), Some(&result(3, 1)));
}

#[test]
fn evicting_a_result_unfolds_and_invalidates_the_retained_call() {
    let original = call(1);
    let empty = entry(9, "");
    let filler_bytes =
        crate::rows::MAX_ROW_BYTES - crate::rows::weight(&original) - crate::rows::weight(&empty);
    let filler = entry(9, &"x".repeat(filler_bytes));
    let mut rows = crate::rows::Rows::new(vec![result(3, 1), original, filler]).expect("rows");
    assert!(rows.has_tool_result(&entry_id(1)));
    let mut changed = Vec::new();
    assert_eq!(rows.bound(true, &mut changed).expect("source budget"), 1);
    assert!(rows.get(&entry_id(1)).is_some());
    assert!(!rows.has_tool_result(&entry_id(1)));
    assert_eq!(changed, vec![entry_id(1)]);
}
