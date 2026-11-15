//! Keep field edits, defaults and neutral answers independent of any embedded session owner.

use carlos_proto::commands::ApprovalAnswer;
use carlos_proto::session::{ApprovalInput, Backend};
use serde_json::{json, Value};

use crate::app::claude_question_state::NativeDraft;
use crate::app::remote::questions_answers::answer;

fn field_input(key: &str, multiple: bool, options: Value, notes_only: bool) -> ApprovalInput {
    serde_json::from_value(json!({"kind":"fields","fields":[{
        "key":key,"title":"Choice","description":"Which?","secret":false,
        "multiple":multiple,"other":true,"notes_only":notes_only,"required":false,
        "schema":{"type":"string"},"options":options}]}))
    .expect("checked typed field")
}

fn content(input: &ApprovalInput, draft: &NativeDraft, backend: Backend) -> Value {
    match answer(input, draft, Some(backend)).expect("neutral typed answer") {
        ApprovalAnswer::Input { value } => value,
        _ => panic!("field input never becomes a plain decision"),
    }
}

#[test]
fn claude_single_multiple_and_other_fields_preserve_selected_values_and_whitespace() {
    for (multiple, choose_other, expected_values, expected_text) in [
        (false, false, json!(["A"]), Value::Null),
        (true, false, json!(["A", "B"]), Value::Null),
        (true, true, json!(["A", "B"]), json!(" custom\nanswer ")),
        (false, true, json!([]), json!(" custom\nanswer ")),
    ] {
        let input = field_input(
            "Which?",
            multiple,
            json!([{"value":"A","label":"A","description":""},
                {"value":"B","label":"B","description":""}]),
            false,
        );
        let mut draft = NativeDraft::from_input(&input);
        let field = &mut draft.fields[0];
        field.select();
        if multiple {
            field.cursor = 1;
            field.select();
        }
        if choose_other {
            field.cursor = 2;
            field.select();
            field.paste(" custom\nanswer ");
        }
        assert_eq!(
            content(&input, &draft, Backend::Claude),
            json!({"fields":[{
            "key":"Which?","values":expected_values,"text":expected_text}]})
        );
    }
}

fn codex_field_draft(
    input: &ApprovalInput,
    selected: bool,
    text: &str,
    omitted: bool,
) -> NativeDraft {
    let mut draft = NativeDraft::from_input(input);
    let field = &mut draft.fields[0];
    if selected {
        field.select();
    }
    field.use_text = !text.is_empty();
    field.editor.insert_str(text);
    field.omitted = omitted;
    draft
}

#[test]
fn codex_fields_keep_choice_values_raw_notes_and_explicit_omission_for_the_owner() {
    for (options, is_other, selected, text, omitted, expected_values, expected_text) in [
        (
            Value::Null,
            false,
            false,
            " free\ntext ",
            false,
            json!([]),
            json!(" free\ntext "),
        ),
        (
            json!([{"value":"A","label":"A","description":""}]),
            false,
            true,
            "",
            false,
            json!(["A"]),
            Value::Null,
        ),
        (
            json!([{"value":"A","label":"A","description":""}]),
            false,
            true,
            " note ",
            false,
            json!(["A"]),
            json!(" note "),
        ),
        (
            json!([{"value":"A","label":"A","description":""}]),
            true,
            false,
            " other ",
            false,
            json!([]),
            json!(" other "),
        ),
        (Value::Null, false, false, "", true, json!([]), Value::Null),
    ] {
        let notes = options.is_array() && !is_other;
        let input = field_input(
            "native-q-7",
            false,
            if options.is_null() {
                json!([])
            } else {
                options
            },
            notes,
        );
        let draft = codex_field_draft(&input, selected, text, omitted);
        assert_eq!(
            content(&input, &draft, Backend::Codex),
            json!({"fields":[{
            "key":"native-q-7","values":expected_values,"text":expected_text}]})
        );
    }
}

fn defaults() -> Vec<(Value, Value)> {
    vec![
        (
            json!({"type":"string","default":"line\n日本語"}),
            json!("line\n日本語"),
        ),
        (json!({"type":"number","default":1.25}), json!(1.25)),
        (json!({"type":"integer","default":-7}), json!(-7)),
        (json!({"type":"boolean","default":false}), json!(false)),
        (
            json!({"type":"string","enum":["a","b"],"enumNames":["A","B"],"default":"b"}),
            json!("b"),
        ),
        (
            json!({"type":"array","items":{"enum":["a","b"]},"default":["b","a"]}),
            json!(["a", "b"]),
        ),
        (
            json!({"type":"string","oneOf":[{"const":"x","title":"X"}],"default":"x"}),
            json!("x"),
        ),
        (
            json!({"type":"string","anyOf":[{"const":"y","title":"Y"}],"default":"y"}),
            json!("y"),
        ),
    ]
}

#[test]
fn every_mcp_field_kind_keeps_its_typed_default_and_optional_omit_in_both_backends() {
    let mut errors = Vec::new();
    for (rule, expected) in defaults() {
        let input = ApprovalInput::Form {
            schema: json!({"type":"object",
            "properties":{"value":rule,"omitted":{"type":"string"}}}),
        };
        let draft = NativeDraft::from_input(&input);
        for backend in [Backend::Claude, Backend::Codex] {
            if let Err(error) = answer(&input, &draft, Some(backend)) {
                errors.push(format!("{backend:?} {input:?}: {error}"));
                continue;
            }
            assert_eq!(content(&input, &draft, backend), json!({"value":expected}));
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn typed_form_descriptions_keep_original_parser_errors_and_empty_choices_allow_text() {
    for (schema, expected) in [
        (json!({}), "Missing form properties"),
        (
            json!({"properties":{"x":{"enum":[1]}}}),
            "Non-string choice",
        ),
    ] {
        let input = ApprovalInput::Form { schema };
        let draft = NativeDraft::from_input(&input);
        assert_eq!(draft.unsupported.as_deref(), Some(expected));
        for backend in [Backend::Claude, Backend::Codex] {
            assert_eq!(
                answer(&input, &draft, Some(backend)).err().as_deref(),
                Some(expected)
            );
        }
    }
    let input = field_input("Q", false, json!([]), false);
    let mut draft = NativeDraft::from_input(&input);
    assert!(draft.unsupported.is_none());
    draft.fields[0].paste("Other");
    assert_eq!(
        content(&input, &draft, Backend::Claude),
        json!({"fields":[{
        "key":"Q","values":[],"text":"Other"}]})
    );
}
