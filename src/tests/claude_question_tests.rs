//! Check pure native question editors without constructing an embedded session owner.

use carlos_core::session::text::safe_label;
use serde_json::json;

use super::claude_question_state::NativeDraft;

#[test]
fn native_plan_without_text_has_no_accepting_editor_answer() {
    let payload =
        json!({ "toolName": "ExitPlanMode", "input": { "planFilePath": "/tmp/missing.md" } });
    let draft = NativeDraft::from_payload("permission", &payload);
    assert!(draft
        .answer(&payload)
        .expect_err("missing plan must fail")
        .contains("no plan text"));
}

#[test]
fn labels_escape_controls_while_native_answers_retain_the_original_value() {
    let payload = json!({ "toolName": "AskUserQuestion", "input": { "questions": [{
        "question": "Which?", "header": "Choice", "multiSelect": false,
        "options": [{ "label": "Line\n\u{1b}[2J", "description": "Choose explicitly" }, { "label": "Other choice", "description": "Alternative" }]
    }] } });
    let mut draft = NativeDraft::from_payload("permission", &payload);
    draft.fields[0].select();
    assert_eq!(
        safe_label(draft.fields[0].spec.options()[0].label()),
        "Line\\n\\u{1b}[2J"
    );
    assert_eq!(
        draft.answer(&payload).expect("native answer")["updatedInput"]["answers"]["Which?"],
        "Line\n\u{1b}[2J"
    );
}
