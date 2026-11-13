//! Entry lookup must reject retired or foreign ids across gaps and history reordering.

use uuid::Uuid;

use super::super::entry_id::EntryId;
use super::super::models::{EntryDraft, Role};
use super::Transcript;

#[test]
fn counter_gaps_from_cards_and_truncation_never_redirect_old_entry_ids() {
    let mut transcript = Transcript::new();
    let startup_card = transcript.allocate_card();
    let first = transcript.append(row("first"));
    let removed = transcript.append(row("removed"));
    let tail = transcript.append(row("tail"));
    assert!(transcript.truncate_before(removed));
    let replacement = transcript.append(row("replacement"));
    let card = transcript.allocate_card();
    let after_card = transcript.append(row("after card"));
    assert_eq!(transcript.position(first), Some(0));
    assert_eq!(transcript.position(replacement), Some(1));
    assert_eq!(transcript.position(after_card), Some(2));
    for retired in [startup_card, removed, tail, card] {
        assert!(transcript.entry(retired).is_none());
    }
    transcript.assert_invariants();
}

#[test]
fn a_foreign_id_with_the_same_counter_cannot_resolve_to_an_ordered_entry() {
    let mut transcript = Transcript::new();
    let id = transcript.append(row("first"));
    let foreign = EntryId::for_approval_test(Uuid::nil(), 0);
    assert_ne!(id, foreign);
    assert!(transcript.position(foreign).is_none());
    assert_eq!(transcript.entry(id).expect("live entry").text, "first");
}

#[test]
fn an_empty_truncated_store_still_rejects_old_ids_after_later_appends() {
    let mut transcript = Transcript::new();
    let old = transcript.append(row("old"));
    assert!(transcript.truncate_before(old));
    let new = transcript.append(row("new"));
    assert!(transcript.position(old).is_none());
    assert_eq!(transcript.position(new), Some(0));
    transcript.assert_invariants();
}

fn row(text: &str) -> EntryDraft {
    EntryDraft::plain(Role::Assistant, text)
}
