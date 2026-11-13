//! Check unfinished rows through empty placeholders, completion and retired suffixes.

use super::Transcript;
use crate::backend::BackendKind;
use crate::session::entry_key::{BackendEntryKey, NativeEntryKey};
use crate::session::models::{EntryDraft, Role};

fn key(item: &str) -> BackendEntryKey {
    BackendEntryKey {
        backend: BackendKind::Codex,
        conversation: "root".into(),
        turn: Some("turn".into()),
        item: NativeEntryKey::Item(item.into()),
        part: 0,
    }
}

#[test]
fn unfinished_placeholders_become_visible_without_hiding_earlier_work() {
    let mut rows = Transcript::new();
    let empty = rows.append(EntryDraft::plain(Role::Reasoning, ""));
    let visible = rows.append(EntryDraft::plain(Role::Reasoning, "earlier"));
    let latest = rows.append(EntryDraft::plain(Role::Reasoning, "latest"));
    for id in [empty, visible, latest] {
        rows.activate(id);
    }
    rows.bind_item(key("visible"), visible);
    rows.bind_item(key("latest"), latest);
    assert_eq!(rows.active_step(0), Some(2));
    rows.complete(&key("latest"));
    assert_eq!(rows.active_step(0), Some(1));
    assert_eq!(rows.active_step(2), None);
    rows.complete(&key("visible"));
    assert_eq!(rows.active_step(0), None);
    rows.append_text(0, "now visible", None);
    assert_eq!(rows.active_step(0), Some(0));
    rows.assert_invariants();
}

#[test]
fn a_retired_active_id_cannot_finish_or_reactivate_its_replacement() {
    let mut rows = Transcript::new();
    let first = rows.append(EntryDraft::plain(Role::Reasoning, "retained"));
    let retired = rows.append(EntryDraft::plain(Role::Reasoning, "retired"));
    rows.bind_item(key("retired"), retired);
    rows.activate(first);
    rows.activate(retired);
    assert!(rows.truncate_before(retired));
    let replacement = rows.append(EntryDraft::plain(Role::Reasoning, "replacement"));
    assert_ne!(replacement, retired);
    rows.bind_item(key("replacement"), replacement);
    rows.activate(retired);
    assert_eq!(rows.active_step(0), Some(0));
    rows.activate(replacement);
    rows.complete(&key("retired"));
    assert_eq!(rows.active_step(0), Some(1));
    rows.complete(&key("replacement"));
    assert_eq!(rows.active_step(0), Some(0));
    assert!(!rows.is_complete(first));
    assert!(rows.is_complete(replacement));
    assert!(!rows.is_complete(retired));
    rows.assert_invariants();
}
