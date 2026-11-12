//! Enumerate parser guards with the same public editor builders as the old code.

use serde_json::{json, Value};

use crate::app::claude_question_state::NativeDraft;
use crate::app::codex_questions::build;

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
        let draft = NativeDraft::from_payload(
            "permission",
            &json!({"toolName":"AskUserQuestion","input":input}),
        );
        assert_eq!(draft.unsupported.as_deref(), Some(expected));
        assert!(draft.fields.is_empty());
    }
}

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

#[test]
fn incomplete_choice_metadata_remains_accepted_in_mcp_forms() {
    for rule in [
        json!({"type":"string","enum":[17],"enumNames":["Seventeen"],"default":17}),
        json!({"type":"string","oneOf":[{"const":17}],"default":17}),
        json!({"type":"string","anyOf":[{"const":17}],"default":17}),
    ] {
        let payload = json!({"requestedSchema":{"properties":{"value":rule}}});
        let draft = NativeDraft::from_payload("elicitation", &payload);
        assert_eq!(
            draft.answer(&payload).expect("existing tolerant choice"),
            json!({"action":"accept","content":{"value":17}})
        );
    }
}
