//! Preserve exact card-local values and private text while rejecting foreign, edited and retired questions.

use std::sync::Arc;

use serde_json::{json, Value};

use crate::test_support::{connected, info, reply, request, SESSION};
use crate::{
    PhoneApprovalInput, PhoneClient, PhoneDecision, PhoneError, PhoneQuestionAnswer, PhoneQuestions,
};

const CARD: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb-0000000000000001";

fn card(input: Value) -> Value {
    let choices = if input["kind"] == "browser" {
        json!(["once", "deny", "cancel"])
    } else {
        json!(["deny", "cancel"])
    };
    json!({"id":CARD,"kind":"question","title":"Which?","details":[],
        "choices":choices,"active":true,"submitted":false,"input":input})
}

fn input() -> Value {
    json!({"kind":"fields","fields":[{
        "key":"native-key","title":"Choice","description":"Pick values and add text",
        "secret":false,"multiple":true,"other":true,"notes_only":false,"required":true,
        "schema":{"type":"string"},"options":[
            {"value":17,"label":"First, with comma","description":"Original numeric value"},
            {"value":true,"label":"Second","description":"Original boolean value"}]
    }]})
}

fn client(input: Value) -> Arc<PhoneClient> {
    let client = connected();
    let frame = client.subscribe(SESSION.into()).expect("subscribe");
    reply(
        &client,
        &frame,
        json!({"kind":"snapshot","snapshot":{
            "info":info("starting"),"seq":"1","has_older":false,"entries":[],"approvals":[card(input)]
        }}),
    );
    client
}

fn questions(client: &PhoneClient) -> Arc<PhoneQuestions> {
    match client
        .approvals(SESSION.into())
        .expect("cards")
        .pop()
        .expect("question")
        .input
    {
        Some(PhoneApprovalInput::Fields { questions }) => questions,
        _ => panic!("expected typed question"),
    }
}

fn answer(selected: Vec<u32>, text: Option<String>) -> Vec<PhoneQuestionAnswer> {
    vec![PhoneQuestionAnswer {
        key: "native-key".into(),
        selected,
        text,
    }]
}

fn replace(client: &PhoneClient, seq: &str, event: Value) {
    client
        .receive(
            1,
            serde_json::to_vec(&json!({"jsonrpc":"2.0","method":"session/event",
        "params":{"session":SESSION,"seq":seq,"event":event}}))
            .expect("event bytes"),
        )
        .expect("checked event");
}

#[test]
fn starting_questions_keep_exact_values_text_and_original_card_without_fake_resolution() {
    let client = client(input());
    let captured = questions(&client);
    assert!(captured.same_questions(questions(&client)));
    let fields = captured.fields();
    assert_eq!(fields[0].options[0].label, "First, with comma");
    let submission = client
        .answer_questions(captured, answer(vec![1, 0], Some(" private\n🦀 ".into())))
        .expect("original questions");
    let command = request(&submission.frame);
    assert_eq!(command["params"]["approval"], CARD);
    assert_eq!(
        command["params"]["answer"],
        json!({"kind":"input","value":{"fields":[{
            "key":"native-key","values":[true,17],"text":" private\n🦀 "
        }]}})
    );
    assert_eq!(
        client
            .approvals(SESSION.into())
            .expect("no fake resolution")
            .len(),
        1
    );
}

#[test]
fn malformed_choices_and_oversized_text_admit_no_operation_and_never_echo_private_text() {
    let client = client(input());
    let captured = questions(&client);
    for values in [vec![2], vec![0, 0]] {
        assert!(matches!(
            client.answer_questions(captured.clone(), answer(values, None)),
            Err(PhoneError::InvalidInput { .. })
        ));
    }
    let failure = client
        .answer_questions(
            captured.clone(),
            answer(vec![], Some("PRIVATE".repeat(10000))),
        )
        .err()
        .expect("bounded answer");
    assert!(!failure.to_string().contains("PRIVATE"));
    assert!(client.answer_questions(captured, vec![]).is_err());
    assert!(client.operations().expect("nothing admitted").is_empty());
}

#[test]
fn a_foreign_host_cannot_submit_even_identical_question_addresses() {
    let first = client(input());
    let other = client(input());
    let captured = questions(&first);
    assert!(!captured.same_questions(questions(&other)));
    assert!(matches!(
        other.answer_questions(captured, answer(vec![0], None)),
        Err(PhoneError::InvalidInput { .. })
    ));
    assert!(other.operations().expect("no redirected work").is_empty());
}

#[test]
fn edited_or_retired_questions_never_reinterpret_old_selected_positions() {
    let client = client(input());
    let captured = questions(&client);
    let mut changed = input();
    changed["fields"][0]["options"][0]["value"] = json!("different value at same position");
    replace(
        &client,
        "2",
        json!({"kind":"approval_requested","approval":card(changed),"before":null}),
    );
    assert!(!captured.same_questions(questions(&client)));
    assert!(matches!(
        client.answer_questions(captured.clone(), answer(vec![0], None)),
        Err(PhoneError::StaleRead { .. })
    ));
    replace(
        &client,
        "3",
        json!({"kind":"approval_resolved","approval":CARD,"decision":null}),
    );
    assert!(matches!(
        client.answer_questions(captured, answer(vec![0], None)),
        Err(PhoneError::StaleRead { .. })
    ));
    assert!(client.operations().expect("no stale work").is_empty());
}

#[test]
fn offline_questions_keep_the_captured_editor_but_cannot_submit_cached_facts() {
    let client = client(input());
    let captured = questions(&client);
    client.disconnect(1).expect("disconnect");
    assert!(matches!(
        client.answer_questions(captured, answer(vec![0], None)),
        Err(PhoneError::StaleRead { .. })
    ));
    assert!(client.operations().expect("no offline work").is_empty());
}

#[test]
fn browser_completion_never_submits_consent_without_an_explicit_offered_decision() {
    let client = client(json!({"kind":"browser","url":"https://example.test/consent"}));
    assert!(client
        .operations()
        .expect("no automatic consent")
        .is_empty());
    let result = client
        .approve(SESSION.into(), CARD.into(), PhoneDecision::Once)
        .expect("explicit consent");
    assert_eq!(
        request(&result.frame)["params"]["answer"],
        json!({"kind":"decision","decision":"once"})
    );
}
