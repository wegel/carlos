//! Enumerate Claude question parser failures in their original order.

use serde_json::{json, Value};

use super::ask_fields;

#[test]
fn claude_question_errors_keep_the_original_checks_and_order() {
    for (input, expected) in [
        (Value::Null, "No questions supplied"),
        (json!({"questions":[]}), "No questions supplied"),
        (json!({"questions":[{}]}), "Question text is missing"),
        (
            json!({"questions":[{"question":"Q"}]}),
            "Question choices are missing",
        ),
        (
            json!({"questions":[{"question":"Q","options":[{}]}]}),
            "Choice label is missing",
        ),
        (
            json!({"questions":[{"question":"Q","options":[]},{"question":"Q"}]}),
            "Duplicate question text",
        ),
    ] {
        let error = ask_fields(&input).err().expect("invalid question");
        assert_eq!(error, expected);
    }
}
