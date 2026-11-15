//! Keep old approval fixtures blocking unless the daemon explicitly exports a nonblocking card.

use serde_json::{json, Value};

use crate::session::Approval;

fn old_card() -> Value {
    json!({"id":format!("{}-0000000000000001", "a".repeat(32)), "kind":"question", "title":"Query",
        "details":[],"choices":["deny"],"input":null,"submitted":false,"active":true})
}

#[test]
fn legacy_approval_defaults_to_blocking_and_omits_the_default_on_export() {
    let card: Approval = serde_json::from_value(old_card()).expect("legacy card");
    assert!(card.blocking);
    assert!(serde_json::to_value(&card)
        .expect("typed export")
        .get("blocking")
        .is_none());
}

#[test]
fn nonblocking_approval_survives_a_checked_round_trip() {
    let mut value = old_card();
    value["blocking"] = json!(false);
    let card: Approval = serde_json::from_value(value).expect("nonblocking card");
    assert!(!card.blocking);
    let value = serde_json::to_value(&card).expect("typed export");
    assert_eq!(value["blocking"], false);
    assert_eq!(
        serde_json::from_value::<Approval>(value).expect("same card"),
        card
    );
}
