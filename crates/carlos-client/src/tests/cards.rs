//! Apply card moves only after their anchors exist and keep malformed updates out of the live copy.

use carlos_proto::events::Event;
use carlos_proto::session::Approval;
use serde_json::json;

use super::{entry_id, event, snapshot};
use crate::Replica;

fn card(id: u64) -> Approval {
    serde_json::from_value(json!({"id":entry_id(id), "kind":"command", "title":"Run?",
        "details":[], "choices":["once","deny"], "input":null,
        "submitted":false, "active":false, "default_to_no":false}))
    .expect("card")
}

#[test]
fn reverse_card_events_keep_order_and_reorders_need_no_changed_card_text() {
    let mut initial = snapshot();
    initial.approvals.push(card(3));
    let mut replica = Replica::new(initial).expect("snapshot");
    replica
        .apply(event(
            11,
            Event::ApprovalRequested {
                approval: card(2),
                before: Some(entry_id(3)),
            },
        ))
        .expect("second before third");
    replica
        .apply(event(
            12,
            Event::ApprovalRequested {
                approval: card(1),
                before: Some(entry_id(2)),
            },
        ))
        .expect("first before second");
    assert_eq!(replica.approvals(), [card(1), card(2), card(3)]);
    replica
        .apply(event(
            13,
            Event::ApprovalRequested {
                approval: card(2),
                before: None,
            },
        ))
        .expect("move second to tail");
    assert_eq!(replica.approvals(), [card(1), card(3), card(2)]);
}

#[test]
fn missing_anchors_and_body_collisions_leave_existing_cards_untouched() {
    let mut initial = snapshot();
    initial.approvals.push(card(3));
    let mut replica = Replica::new(initial).expect("snapshot");
    assert!(replica
        .apply(event(
            11,
            Event::ApprovalRequested {
                approval: card(1),
                before: Some(entry_id(2)),
            }
        ))
        .is_err());
    assert!(replica.is_stale());
    assert_eq!(replica.approvals(), [card(3)]);
    replica.replace(snapshot()).expect("refresh");
    assert!(replica
        .apply(event(
            11,
            Event::ApprovalRequested {
                approval: card(10),
                before: None,
            }
        ))
        .is_err());
    assert!(replica.approvals().is_empty());
}

#[test]
fn caller_labels_accept_exact_utf8_bound_and_reject_oversized_updates_without_replacement() {
    let mut accepted = card(3);
    accepted.caller = Some("🌍".repeat(1024));
    let mut initial = snapshot();
    initial.approvals.push(accepted.clone());
    let mut replica = Replica::new(initial).expect("exact 4096-byte caller");
    let mut oversized = accepted.clone();
    oversized.caller.as_mut().expect("caller").push('a');
    assert!(replica
        .apply(event(
            11,
            Event::ApprovalRequested {
                approval: oversized.clone(),
                before: None,
            }
        ))
        .is_err());
    assert_eq!(replica.approvals(), [accepted]);
    let mut initial = snapshot();
    initial.approvals.push(oversized);
    assert!(Replica::new(initial).is_err());
}

#[test]
fn preflight_error_metadata_obeys_its_utf8_byte_bound_without_replacing_good_facts() {
    let mut accepted = card(3);
    accepted.input_error = Some("🌍".repeat(512));
    let mut initial = snapshot();
    initial.approvals.push(accepted.clone());
    let mut replica = Replica::new(initial).expect("exact 2048-byte preflight error");
    let mut oversized = accepted.clone();
    oversized.input_error.as_mut().expect("error").push('a');
    assert!(replica
        .apply(event(
            11,
            Event::ApprovalRequested {
                approval: oversized,
                before: None,
            }
        ))
        .is_err());
    assert_eq!(replica.approvals(), [accepted]);
}

#[test]
fn request_captions_obey_their_byte_bound_and_keep_the_opaque_card_address() {
    let mut accepted = card(3);
    accepted.display_id = Some("🌍".repeat(1024));
    let mut initial = snapshot();
    initial.approvals.push(accepted.clone());
    let mut replica = Replica::new(initial).expect("exact 4096-byte display caption");
    let mut oversized = accepted.clone();
    oversized.display_id.as_mut().expect("caption").push('a');
    assert!(replica
        .apply(event(
            11,
            Event::ApprovalRequested {
                approval: oversized,
                before: None
            }
        ))
        .is_err());
    assert_eq!(replica.approvals(), [accepted]);
}
