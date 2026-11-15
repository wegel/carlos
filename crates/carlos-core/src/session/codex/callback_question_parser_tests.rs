//! Keep the original question-parser guard order in the native owner.

use serde_json::{json, Value};

use super::callback_questions::build;

#[test]
fn codex_question_errors_keep_the_original_checks_and_order() {
    for (questions, expected) in [
        (Value::Null, "Expected 1 to64 questions"),
        (json!([]), "Expected 1 to64 questions"),
        (json!(vec![json!({}); 65]), "Expected 1 to64 questions"),
        (json!([{}]), "Question has no native ID"),
        (json!([{"id":""}]), "Question has no native ID"),
        (json!([{"id":"x"}]), "Question text is missing"),
        (
            json!([{"id":"x","question":"Q"}]),
            "Question header is missing",
        ),
        (
            json!([{"id":"x","question":"Q","header":"H","options":[{}]}]),
            "Choice label is missing",
        ),
        (
            json!([{"id":"x","question":"Q","header":"H"},{"id":"x"}]),
            "Question IDs repeat",
        ),
    ] {
        let (_, draft) = build(
            "item/tool/requestUserInput",
            &json!({"threadId":"root","questions":questions}),
            json!("q"),
        )
        .expect("unsupported editor");
        assert_eq!(draft.unsupported.as_deref(), Some(expected));
        assert!(draft.fields.is_empty());
    }
}
