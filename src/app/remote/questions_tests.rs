//! Keep local focus, confirmations and private fields independent from shared callback metadata.

use carlos_proto::ids::EntryId;
use carlos_proto::session::{Approval, ApprovalInput};
use serde_json::json;

use super::Questions;

fn id(index: u64) -> EntryId {
    format!("{}-{index:016x}", "a".repeat(32))
        .parse()
        .expect("opaque callback card")
}

fn card(index: u64) -> Approval {
    serde_json::from_value(json!({"id":id(index),"kind":"question","title":"Original question",
        "caller":"Thread: original", "details":[], "choices":["deny","cancel"], "submitted":false,
        "active":index == 1, "blocking":false,"default_to_no":true,
        "input":{"kind":"fields","fields":[{"key":"original-field","title":"Field","description":"Original prompt",
            "secret":false,"multiple":false,"other":true,"notes_only":false,"required":false,
            "schema":{"type":"string"},"options":[]}]}})).expect("typed callback card")
}

fn insert(state: &mut Questions, index: u64, value: &str) {
    state
        .drafts
        .get_mut(&id(index))
        .expect("private draft")
        .editor
        .fields[0]
        .editor
        .insert_str(value);
}

fn text(state: &Questions, index: u64) -> String {
    state.drafts[&id(index)].editor.fields[0]
        .editor
        .lines()
        .join("\n")
}

#[test]
fn two_devices_cycle_focus_without_changing_shared_active_cards_or_other_drafts() {
    let cards = [card(1), card(2)];
    let original = cards.clone();
    let (mut first, mut second) = (Questions::default(), Questions::default());
    first.synchronize(&cards);
    second.synchronize(&cards);
    assert!(!first.shown && !second.shown);
    first.next(&cards);
    second.next(&cards);
    second.next(&cards);
    insert(&mut first, 1, "first private answer");
    insert(&mut second, 2, "second private answer");
    assert_eq!(first.focus, Some(id(1)));
    assert_eq!(second.focus, Some(id(2)));
    first.synchronize(&cards);
    second.synchronize(&cards);
    assert_eq!(text(&first, 1), "first private answer");
    assert_eq!(text(&second, 2), "second private answer");
    assert_eq!(cards, original);
}

#[test]
fn readonly_caller_and_browser_progress_details_keep_private_confirmation_and_answer() {
    let mut card = card(1);
    let mut state = Questions::default();
    state.synchronize(&[card.clone()]);
    insert(&mut state, 1, "answer stays private");
    state.drafts.get_mut(&id(1)).expect("draft").confirm = true;
    card.details
        .push("The server reported browser completion; consent is still required".into());
    card.caller = Some("Thread: same original caller with updated display metadata".into());
    state.synchronize(&[card]);
    assert_eq!(text(&state, 1), "answer stays private");
    assert!(state.drafts[&id(1)].confirm);
}

#[test]
fn changed_input_on_a_live_id_retires_the_old_editor_until_explicit_focus() {
    let mut card = card(1);
    let mut state = Questions::default();
    state.synchronize(&[card.clone()]);
    state.next(&[card.clone()]);
    insert(&mut state, 1, "never redirect this answer");
    let Some(ApprovalInput::Fields { fields }) = &mut card.input else {
        panic!("field card")
    };
    fields[0].key = "replacement-field".into();
    state.synchronize(&[card.clone()]);
    assert!(!state.shown);
    assert!(!state.drafts.contains_key(&id(1)));
    state.next(&[card]);
    assert!(state.shown);
    assert_eq!(text(&state, 1), "");
    assert_eq!(
        state.drafts[&id(1)].editor.fields[0].spec.name(),
        "replacement-field"
    );
}

#[test]
fn retiring_one_card_keeps_another_cards_editor_and_explicit_focus() {
    let cards = [card(1), card(2)];
    let mut state = Questions::default();
    state.synchronize(&cards);
    state.next(&cards);
    state.next(&cards);
    insert(&mut state, 2, "surviving private answer");
    state.synchronize(&cards[1..]);
    assert_eq!(state.focus, Some(id(2)));
    assert!(!state.drafts.contains_key(&id(1)));
    assert_eq!(text(&state, 2), "surviving private answer");
}

#[test]
fn accepted_receipt_retires_with_its_exact_card_in_either_reply_order() {
    let cards = [card(1), card(2)];
    let mut state = Questions::default();
    let mut hint = None;
    state.accept_receipt(&id(1), "accepted first".into(), &cards, &mut hint);
    state.retire_receipt(&cards[..1], &mut hint);
    assert_eq!(hint.as_deref(), Some("accepted first"));
    state.retire_receipt(&cards[1..], &mut hint);
    assert!(hint.is_none());
    state.accept_receipt(&id(1), "late accepted".into(), &cards[1..], &mut hint);
    assert!(
        hint.is_none(),
        "a late success must not mask current status"
    );
    state.accept_receipt(&id(2), "accepted second".into(), &cards[1..], &mut hint);
    state.retire_receipt(&[], &mut hint);
    assert!(hint.is_none());
}

#[test]
fn retired_receipt_preserves_a_newer_hint_and_other_cards_private_draft() {
    let cards = [card(1), card(2)];
    let mut state = Questions::default();
    let mut hint = None;
    state.synchronize(&cards);
    insert(&mut state, 2, "another private answer");
    state.accept_receipt(&id(1), "accepted".into(), &cards, &mut hint);
    hint = Some("newer unrelated failure".into());
    state.retire_receipt(&cards[1..], &mut hint);
    state.synchronize(&cards[1..]);
    assert_eq!(hint.as_deref(), Some("newer unrelated failure"));
    assert_eq!(text(&state, 2), "another private answer");
}

#[test]
fn an_explicit_retry_clears_only_the_same_cards_previous_error_hint() {
    let mut state = Questions::default();
    state.synchronize(&[card(1)]);
    state.drafts.get_mut(&id(1)).expect("draft").editor.error = Some("own rejection".into());
    let mut hint = Some("newer unrelated failure".into());
    state.clear_retry_hint(&id(1), &mut hint);
    assert_eq!(hint.as_deref(), Some("newer unrelated failure"));
    hint = Some("own rejection".into());
    state.clear_retry_hint(&id(2), &mut hint);
    assert_eq!(hint.as_deref(), Some("own rejection"));
    state.clear_retry_hint(&id(1), &mut hint);
    assert!(hint.is_none());
}
