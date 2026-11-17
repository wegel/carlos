//! Preserve offered decisions and captured turns without inventing local success facts.

use std::sync::Arc;

use serde_json::json;

use crate::test_support::{connected, info, reply, request, SESSION};
use crate::{PhoneApprovalInput, PhoneClient, PhoneDecision, PhoneError, PhonePhase};

const CARD: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb-0000000000000001";
const TURN: &str = "cccccccccccccccccccccccccccccccc";

fn with_card(starting: bool, input: bool) -> Arc<PhoneClient> {
    let client = connected();
    let mut header = info(if starting { "starting" } else { "ready" });
    header["phase"] = "approval".into();
    header["current_turn"] = TURN.into();
    let frame = client.subscribe(SESSION.into()).expect("subscribe");
    reply(
        &client,
        &frame,
        json!({"kind":"snapshot","snapshot":{
            "info":header,"seq":"1","has_older":false,"entries":[],"approvals":[{
                "id":CARD,"kind":"command","title":"Read file","details":["cat file"],
                "choices":["once","deny"],"active":true,"submitted":false,
                "input":if input { json!({"kind":"unsupported","reason":"native input"}) } else { json!(null) }
            }]
        }}),
    );
    client
}

#[test]
fn a_starting_card_accepts_only_its_actual_decisions_and_keeps_prompts_blocked() {
    let client = with_card(true, false);
    let cards = client.approvals(SESSION.into()).expect("cards");
    assert_eq!(cards[0].id, CARD);
    assert!(!cards[0].choices.contains(&PhoneDecision::Session));
    assert!(matches!(
        client.send_prompt(SESSION.into(), "private draft".into()),
        Err(PhoneError::State { .. })
    ));
    assert!(matches!(
        client.approve(SESSION.into(), CARD.into(), PhoneDecision::Session),
        Err(PhoneError::State { .. })
    ));
    assert!(client.operations().expect("no invalid work").is_empty());
    let submission = client
        .approve(SESSION.into(), CARD.into(), PhoneDecision::Once)
        .expect("offered decision");
    let command = request(&submission.frame);
    assert_eq!(command["params"]["approval"], CARD);
    assert_eq!(
        command["params"]["answer"],
        json!({"kind":"decision","decision":"once"})
    );
    assert!(!client
        .approvals(SESSION.into())
        .expect("no fake resolution")
        .is_empty());
}

#[test]
fn structured_input_cannot_be_replaced_with_an_empty_allow_answer() {
    let client = with_card(false, true);
    assert!(matches!(
        client.approvals(SESSION.into()).expect("cards")[0].input,
        Some(PhoneApprovalInput::Unsupported { .. })
    ));
    assert!(matches!(
        client.approve(SESSION.into(), CARD.into(), PhoneDecision::Once),
        Err(PhoneError::State { .. })
    ));
    assert!(client.operations().expect("not admitted").is_empty());
    let deny = client
        .approve(SESSION.into(), CARD.into(), PhoneDecision::Deny)
        .expect("actual deny choice");
    assert_eq!(request(&deny.frame)["params"]["answer"]["decision"], "deny");
}

#[test]
fn interrupt_captures_the_original_turn_and_never_changes_the_shared_phase_optimistically() {
    let client = with_card(false, false);
    let stop = client.interrupt(SESSION.into()).expect("captured stop");
    let command = request(&stop.frame);
    assert_eq!(command["method"], "session/interrupt");
    assert_eq!(command["params"]["turn"], TURN);
    let metadata = client
        .transcript(SESSION.into())
        .expect("window")
        .expect("metadata");
    assert!(matches!(metadata.info.activity.phase, PhonePhase::Approval));
    assert!(!client
        .approvals(SESSION.into())
        .expect("card remains")
        .is_empty());
    let ralph = client
        .set_ralph(SESSION.into(), true)
        .expect("ralph request");
    assert_eq!(request(&ralph.frame)["params"]["on"], true);
    assert!(
        !client
            .transcript(SESSION.into())
            .expect("window")
            .expect("metadata")
            .info
            .activity
            .ralph
    );
}

#[test]
fn retired_card_cannot_be_answered_after_an_observed_resolution() {
    let client = with_card(false, false);
    client
        .receive(
            1,
            serde_json::to_vec(&json!({"jsonrpc":"2.0","method":"session/event",
            "params":{"session":SESSION,"seq":"2","event":{
                "kind":"approval_resolved","approval":CARD,"decision":null
            }}}))
            .expect("resolution"),
        )
        .expect("checked event");
    assert!(client
        .approvals(SESSION.into())
        .expect("no live card")
        .is_empty());
    assert!(matches!(
        client.approve(SESSION.into(), CARD.into(), PhoneDecision::Once),
        Err(PhoneError::State { .. })
    ));
    assert!(client.operations().expect("no invalid control").is_empty());
}
