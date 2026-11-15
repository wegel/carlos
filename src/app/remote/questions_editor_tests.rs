//! Exercise local editors with checked daemon fields rather than native SDK envelopes.

use carlos_core::session::text::safe_label;
use carlos_proto::commands::ApprovalAnswer;
use carlos_proto::session::{ApprovalInput, Backend};
use serde_json::json;

use crate::app::claude_question_state::NativeDraft;
use crate::app::remote::questions_answers::answer;

#[test]
fn unsupported_plan_has_no_accepting_editor_answer() {
    let input = ApprovalInput::Unsupported {
        reason: "Claude supplied no plan text".into(),
    };
    let draft = NativeDraft::from_approval(Some(&input));
    assert!(answer(&input, &draft, Some(Backend::Claude))
        .expect_err("missing plan must fail")
        .contains("no plan text"));
}

#[test]
fn labels_escape_controls_while_answers_keep_the_original_offered_value() {
    let input: ApprovalInput = serde_json::from_value(json!({"kind":"fields","fields":[{
        "key":"question-key","title":"Choice","description":"Which?",
        "secret":false,"multiple":false,"other":true,"notes_only":false,"required":true,
        "schema":{"type":"string"},"options":[{"value":"original-choice-value",
        "label":"Line\n\u{1b}[2J","description":"Choose explicitly"}]}]}))
    .expect("checked fields");
    let mut draft = NativeDraft::from_approval(Some(&input));
    draft.fields[0].select();
    assert_eq!(
        safe_label(draft.fields[0].spec.options()[0].label()),
        "Line\\n\\u{1b}[2J"
    );
    let ApprovalAnswer::Input { value } =
        answer(&input, &draft, Some(Backend::Claude)).expect("neutral answer")
    else {
        panic!("typed field answer")
    };
    assert_eq!(value["fields"][0]["key"], "question-key");
    assert_eq!(
        value["fields"][0]["values"],
        json!(["original-choice-value"])
    );
}
