//! Preserve complete native answer envelopes at the guarded owner write, without a terminal facade.

use carlos_proto::commands::ApprovalAnswer;
use carlos_proto::session::{ApprovalInput, Decision};
use serde_json::{json, Value};

use super::approval_control_test_support::Peer;
use super::state::SessionState;
use crate::backend::BackendKind;

fn write(
    core: &mut SessionState,
    kind: BackendKind,
    answer: ApprovalAnswer,
) -> Vec<(Value, Value)> {
    let peer = Peer::new(kind);
    let card = core
        .approval()
        .pending()
        .expect("original request")
        .entry_id();
    let work = core
        .prepare_engine_answer(&peer, card, answer)
        .expect("claim original request");
    core.finish_engine_control(&peer, work.execute(&peer.commands))
        .expect("native response");
    let calls = peer.calls.lock().expect("native answers").clone();
    assert_eq!(calls.len(), 1);
    calls
}

fn claude(payload: &Value) -> SessionState {
    let mut core = SessionState::new("root".into(), Default::default(), false);
    let kind = if payload.get("toolName").is_some() {
        "permission"
    } else {
        "elicitation"
    };
    core.receive_claude_callback(&json!({"callbackId":"q","kind":kind,"payload":payload}));
    core
}

#[test]
fn claude_tool_and_browser_answers_keep_the_exact_native_envelopes() {
    for (payload, expected) in [
        (
            json!({"toolName":"Read","input":{"path":"a"}}),
            json!({"behavior":"allow"}),
        ),
        (
            json!({"toolName":"ExitPlanMode","input":{"plan":"Do it"}}),
            json!({"behavior":"allow"}),
        ),
        (
            json!({"mode":"url","url":"https://example.com"}),
            json!({"action":"accept"}),
        ),
    ] {
        let mut core = claude(&payload);
        assert_eq!(
            write(
                &mut core,
                BackendKind::Claude,
                ApprovalAnswer::Decision {
                    decision: Decision::Once
                }
            ),
            vec![(json!("q"), expected)]
        );
    }
}

#[test]
fn claude_single_multiple_and_other_native_answers_preserve_original_input_and_whitespace() {
    for (multiple, other, expected) in [
        (false, false, "A"),
        (true, false, "A, B"),
        (true, true, "A, B,  custom\nanswer "),
        (false, true, " custom\nanswer "),
    ] {
        let payload = json!({"toolName":"AskUserQuestion","input":{"keep":17,"questions":[{
            "question":"Which?","header":"Choice","multiSelect":multiple,
            "options":[{"label":"A"},{"label":"B"}]}]}});
        let mut core = claude(&payload);
        let card = core.export_approvals().remove(0);
        let Some(ApprovalInput::Fields { fields }) = card.input else {
            panic!("offered fields")
        };
        let values: Vec<_> = fields[0]
            .options
            .iter()
            .take(if multiple {
                2
            } else if other {
                0
            } else {
                1
            })
            .map(|option| option.value.clone())
            .collect();
        let answer = ApprovalAnswer::Input {
            value: json!({"fields":[{"key":fields[0].key,
            "values":values,"text":other.then_some(" custom\nanswer ")}]}),
        };
        let mut input = payload["input"].clone();
        input["answers"] = json!({"Which?":expected});
        assert_eq!(
            write(&mut core, BackendKind::Claude, answer),
            vec![(json!("q"), json!({"behavior":"allow","updatedInput":input}))]
        );
    }
}

fn codex(options: Value, other: bool) -> SessionState {
    let mut core = SessionState::new("root".into(), Default::default(), false);
    core.enable_codex();
    core.accept_codex_callback("item/tool/requestUserInput",&json!({"id":"q","params":{
        "threadId":"child","turnId":"turn","itemId":"item","questions":[{
            "id":"native-q-7","header":"Header","question":"Question?","options":options,"isOther":other}]}}));
    core
}

#[test]
fn codex_free_text_choice_notes_other_and_omission_keep_the_exact_native_answers() {
    for (options, other, selected, text, expected) in [
        (
            Value::Null,
            false,
            false,
            " free\ntext ",
            json!(["user_note: free\ntext"]),
        ),
        (json!([{"label":"A"}]), false, true, "", json!(["A"])),
        (
            json!([{"label":"A"}]),
            false,
            true,
            " note ",
            json!(["A", "user_note: note"]),
        ),
        (
            json!([{"label":"A"}]),
            true,
            false,
            " other ",
            json!(["user_note: other"]),
        ),
        (Value::Null, false, false, "", json!([])),
    ] {
        let mut core = codex(options, other);
        let card = core.export_approvals().remove(0);
        let Some(ApprovalInput::Fields { fields }) = card.input else {
            panic!("offered fields")
        };
        let values = if selected {
            vec![fields[0].options[0].value.clone()]
        } else {
            Vec::new()
        };
        let answer = ApprovalAnswer::Input {
            value: json!({"fields":[{"key":fields[0].key,
            "values":values,"text":if text.is_empty() {None} else {Some(text)}}]}),
        };
        assert_eq!(
            write(&mut core, BackendKind::Codex, answer),
            vec![(
                json!("q"),
                json!({"answers":{"native-q-7":{"answers":expected}}})
            )]
        );
    }
}

fn form_cases() -> Vec<(Value, Value)> {
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
fn both_backends_return_every_typed_form_value_without_inventing_optional_fields() {
    for (rule, value) in form_cases() {
        let schema =
            json!({"type":"object","properties":{"value":rule,"omitted":{"type":"string"}}});
        for backend in [BackendKind::Claude, BackendKind::Codex] {
            let mut core = if backend == BackendKind::Claude {
                claude(&json!({"requestedSchema":schema}))
            } else {
                let mut core = SessionState::new("root".into(), Default::default(), false);
                core.enable_codex();
                core.accept_codex_callback("mcpServer/elicitation/request",&json!({"id":"q","params":{
                    "threadId":"root","serverName":"server","mode":"form","requestedSchema":schema}}));
                core
            };
            let content = json!({"value":value});
            assert_eq!(
                write(
                    &mut core,
                    backend,
                    ApprovalAnswer::Input {
                        value: content.clone()
                    }
                ),
                vec![(json!("q"), json!({"action":"accept","content":content}))]
            );
        }
    }
}

#[test]
fn deferred_codex_form_checks_keep_every_original_schema_value_and_revision() {
    for (rule, value) in form_cases() {
        let schema =
            json!({"type":"object","properties":{"value":rule,"omitted":{"type":"string"}}});
        let mut core = SessionState::new("root".into(), Default::default(), false);
        core.enable_codex();
        core.accept_codex_callback(
            "mcpServer/elicitation/request",
            &json!({"id":"form","params":{
            "threadId":"root","serverName":"server","mode":"form","requestedSchema":schema}}),
        );
        let card = core.approval().pending().expect("form card").entry_id();
        let peer = Peer::new(BackendKind::Codex);
        let content = json!({"value":value});
        let expected = json!({"action":"accept","content":content});
        core.answer_codex_callback(
            &peer,
            card,
            &json!("form"),
            expected.clone(),
            crate::session::approval_state::ApprovalChoice::Accept,
        )
        .expect("form worker admitted");
        assert_eq!(
            *peer.checks.lock().expect("original check"),
            vec![json!(["form", schema, content, 1])]
        );
        assert!(peer
            .calls
            .lock()
            .expect("no premature native write")
            .is_empty());
        let action = core
            .checked_codex_callback(&json!({"requestId":"form","revision":1,"ok":true}))
            .action;
        assert!(
            matches!(action, Some(crate::session::actions::ServerRequestAction::ReplyNative {card_id,result})
            if card_id == card && result == expected)
        );
    }
}
