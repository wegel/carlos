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
