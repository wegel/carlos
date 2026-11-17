//! Keep flat form types, optional omission and original schemas across the phone boundary.

use serde_json::{json, Value};

use super::{card, client, questions, replace, request, CARD, SESSION};
use crate::{PhoneApprovalInput, PhoneError, PhoneQuestionAnswer};

fn schema() -> Value {
    json!({"type":"object","required":["count","enabled","mode"],"properties":{
        "count":{"type":"integer"},"enabled":{"type":"boolean","default":true},
        "mode":{"type":"string","oneOf":[{"const":"jp","title":"日本語"},{"const":"fr","title":"Français"}]},
        "optional":{"type":"string"},"many":{"type":"array","items":{"enum":["a,b","🌍"]}}
    }})
}

fn input(schema: Value) -> Value {
    json!({"kind":"form","schema":schema})
}

fn field(key: &str, selected: Vec<u32>, text: Option<&str>) -> PhoneQuestionAnswer {
    PhoneQuestionAnswer {
        key: key.into(),
        selected,
        text: text.map(str::to_owned),
    }
}

fn required(number: &str) -> Vec<PhoneQuestionAnswer> {
    vec![
        field("count", vec![], Some(number)),
        field("enabled", vec![1], None),
        field("mode", vec![1], None),
    ]
}

#[test]
fn typed_form_keeps_large_integers_false_native_enum_values_and_omitted_properties() {
    let client = client(input(schema()));
    let editor = questions(&client);
    assert!(editor.is_form());
    let reply = client
        .answer_questions(editor, required("9007199254740993"))
        .expect("typed values");
    assert_eq!(
        request(&reply.frame)["params"]["answer"],
        json!({"kind":"input","value":{
            "count":9007199254740993_u64,"enabled":false,"mode":"fr"
        }})
    );
}

#[test]
fn explicit_empty_optional_values_are_different_from_omitted_fields() {
    let client = client(input(schema()));
    let mut answers = required("0005");
    answers.push(field("optional", vec![], Some("")));
    answers.push(field("many", vec![], None));
    let reply = client
        .answer_questions(questions(&client), answers)
        .expect("included empty fields");
    let content = &request(&reply.frame)["params"]["answer"]["value"];
    assert_eq!(content["count"], 5);
    assert_eq!(content["optional"], "");
    assert_eq!(content["many"], json!([]));
}

#[test]
fn invalid_numbers_and_missing_required_fields_admit_no_private_operation() {
    let client = client(input(schema()));
    let editor = questions(&client);
    for number in [
        "PRIVATE_BAD_VALUE",
        "NaN",
        "inf",
        "2.5",
        "18446744073709551617",
        "-9223372036854775809",
        "9007199254740993.0",
    ] {
        let failure = client
            .answer_questions(editor.clone(), required(number))
            .err()
            .expect("invalid integer");
        assert!(!failure.to_string().contains("PRIVATE_BAD_VALUE"));
    }
    assert!(matches!(
        client.answer_questions(editor, vec![]),
        Err(PhoneError::InvalidInput { .. })
    ));
    assert!(client.operations().expect("no malformed input").is_empty());
}

#[test]
fn a_changed_form_root_cannot_reuse_a_captured_editor_even_if_field_labels_match() {
    let client = client(input(schema()));
    let editor = questions(&client);
    let mut changed = schema();
    changed["additionalProperties"] = json!(false);
    replace(
        &client,
        "2",
        json!({"kind":"approval_requested","approval":card(input(changed)),"before":null}),
    );
    assert!(!editor.same_questions(questions(&client)));
    assert!(matches!(
        client.answer_questions(editor, required("5")),
        Err(PhoneError::StaleRead { .. })
    ));
    assert!(client
        .operations()
        .expect("old schema cannot submit")
        .is_empty());
}

#[test]
fn unsupported_form_fields_remain_explicit_instead_of_becoming_freeform_allow() {
    let client = client(input(json!({"properties":{"object":{"type":"object"}}})));
    assert!(matches!(
        client.approvals(SESSION.into()).expect("card")[0].input,
        Some(PhoneApprovalInput::Unsupported { .. })
    ));
    assert!(client
        .approve(SESSION.into(), CARD.into(), crate::PhoneDecision::Once)
        .is_err());
    assert!(client
        .operations()
        .expect("no unsupported answer")
        .is_empty());
}
