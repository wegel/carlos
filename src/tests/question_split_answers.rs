//! Enumerate native answer bytes independently of the plain field parser.

use serde_json::{json, Value};

use super::super::codex_question_support::{app, question, receive, Peer};

use super::split_peer::ValidationPeer;
use crate::app::claude_callbacks::receive_callback;
use crate::app::claude_question_state::NativeDraft;
use crate::app::codex_questions::{build, form_answer, question_answer};

use crate::app::turn_approval::respond_to_pending_approval;
use carlos_core::session::approval_state::ApprovalChoice;

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
        let mut app = crate::app::state::AppState::new("session".into());
        let peer = Peer::default();
        let kind = if payload.get("toolName").is_some() {
            "permission"
        } else {
            "elicitation"
        };
        receive_callback(
            &mut app,
            &json!({"callbackId":"q","kind":kind,"payload":payload}),
        );
        respond_to_pending_approval(&peer, &mut app, ApprovalChoice::Accept);
        assert_eq!(*peer.replies.borrow(), vec![(json!("q"), expected)]);
    }
}

#[test]
fn claude_single_multiple_and_other_answers_preserve_input_and_whitespace() {
    for (multiple, choose_other, expected) in [
        (false, false, "A"),
        (true, false, "A, B"),
        (true, true, "A, B,  custom\nanswer "),
        (false, true, " custom\nanswer "),
    ] {
        let payload = json!({"toolName":"AskUserQuestion","input":{"keep":17,"questions":[{
            "question":"Which?","header":"Choice","multiSelect":multiple,
            "options":[{"label":"A"},{"label":"B"}]}]}});
        let mut app = crate::app::state::AppState::new("session".into());
        receive_callback(
            &mut app,
            &json!({"callbackId":"q","kind":"permission","payload":payload}),
        );
        let draft = crate::app::native_questions::draft_mut(&mut app).expect("draft");
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
        let peer = Peer::default();
        respond_to_pending_approval(&peer, &mut app, ApprovalChoice::Accept);
        let mut input = payload["input"].clone();
        input["answers"] = json!({"Which?":expected});
        assert_eq!(
            *peer.replies.borrow(),
            vec![(json!("q"), json!({"behavior":"allow","updatedInput":input}))]
        );
    }
}

#[test]
fn every_mcp_field_kind_keeps_its_typed_value_and_default_in_both_backends() {
    let cases = [
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
    ];
    for (rule, value) in cases {
        let schema =
            json!({"type":"object","properties":{"value":rule,"omitted":{"type":"string"}}});
        let expected = json!({"action":"accept","content":{"value":value}});
        let payload = json!({"requestedSchema":schema});
        let draft = NativeDraft::from_payload("elicitation", &payload);
        assert_eq!(draft.answer(&payload).expect("Claude value"), expected);
        let params =
            json!({"threadId":"root","serverName":"server","mode":"form","requestedSchema":schema});
        let (_, draft) =
            build("mcpServer/elicitation/request", &params, json!("form")).expect("Codex form");
        assert_eq!(form_answer(&draft, &params).expect("Codex value"), expected);
        assert_form_replies(payload, params, expected);
    }
}

#[test]
fn codex_free_text_choice_notes_other_and_omission_keep_exact_answers() {
    for (options, is_other, selected, text, omitted, expected) in [
        (
            Value::Null,
            false,
            false,
            " free\ntext ",
            false,
            json!(["user_note: free\ntext"]),
        ),
        (json!([{"label":"A"}]), false, true, "", false, json!(["A"])),
        (
            json!([{"label":"A"}]),
            false,
            true,
            " note ",
            false,
            json!(["A", "user_note: note"]),
        ),
        (
            json!([{"label":"A"}]),
            true,
            false,
            " other ",
            false,
            json!(["user_note: other"]),
        ),
        (Value::Null, false, false, "", true, json!([])),
    ] {
        let mut app = app();
        let mut frame = question(json!("q"), "child", true);
        frame["params"]["questions"][0]["options"] = options;
        frame["params"]["questions"][0]["isOther"] = json!(is_other);
        receive(&mut app, frame);
        let draft = crate::app::native_questions::draft_mut(&mut app).expect("draft");
        let field = &mut draft.fields[0];
        if selected {
            field.select();
        }
        field.use_text = !text.is_empty();
        field.editor.insert_str(text);
        field.omitted = omitted;
        assert_eq!(
            question_answer(draft).expect("answer"),
            json!({"answers":{"native-q-7":{"answers":expected}}})
        );
        let peer = Peer::default();
        respond_to_pending_approval(&peer, &mut app, ApprovalChoice::Accept);
        assert_eq!(
            *peer.replies.borrow(),
            vec![(
                json!("q"),
                json!({"answers":{"native-q-7":{"answers":expected}}})
            )]
        );
    }
}

#[test]
fn both_backends_keep_parser_errors_and_permissive_defaults() {
    for (schema, expected) in [
        (json!({}), "Missing form properties"),
        (
            json!({"properties":{"x":{"enum":[1]}}}),
            "Non-string choice",
        ),
    ] {
        let payload = json!({"requestedSchema":schema});
        let draft = NativeDraft::from_payload("elicitation", &payload);
        assert_eq!(draft.unsupported.as_deref(), Some(expected));
        let params =
            json!({"threadId":"root","serverName":"s","mode":"form","requestedSchema":schema});
        let (_, draft) =
            build("mcpServer/elicitation/request", &params, json!(1)).expect("unsupported editor");
        assert_eq!(draft.unsupported.as_deref(), Some(expected));
    }
    let payload =
        json!({"toolName":"AskUserQuestion","input":{"questions":[{"question":"Q","options":[]}]}});
    let mut draft = NativeDraft::from_payload("permission", &payload);
    assert!(draft.unsupported.is_none());
    draft.fields[0].paste("Other");
    assert_eq!(
        draft.answer(&payload).expect("empty choices permit Other")["updatedInput"]["answers"],
        json!({"Q":"Other"})
    );
}

fn assert_form_replies(payload: Value, params: Value, expected: Value) {
    let mut claude = crate::app::state::AppState::new("root".into());
    receive_callback(
        &mut claude,
        &json!({"callbackId":"form","kind":"elicitation","payload":payload}),
    );
    let peer = Peer::default();
    respond_to_pending_approval(&peer, &mut claude, ApprovalChoice::Accept);
    assert_eq!(
        *peer.replies.borrow(),
        vec![(json!("form"), expected.clone())]
    );
    let mut codex = app();
    receive(
        &mut codex,
        json!({"id":"form","method":"mcpServer/elicitation/request","params":params}),
    );
    let peer = ValidationPeer::default();
    respond_to_pending_approval(&peer, &mut codex, ApprovalChoice::Accept);
    assert_eq!(
        *peer.checks.lock().expect("form checks"),
        vec![json!([
            "form",
            params["requestedSchema"],
            expected["content"],
            1
        ])]
    );
    let action = crate::app::codex_callbacks::checked(
        &mut codex,
        &json!({"requestId":"form","revision":1,"ok":true}),
    );
    let Some(carlos_core::session::actions::ServerRequestAction::ReplyNative { card_id, result }) =
        action
    else {
        panic!("validated answer must send")
    };
    crate::app::codex_reply::send(&peer, &mut codex, card_id, result);
    assert_eq!(*peer.peer.replies.borrow(), vec![(json!("form"), expected)]);
}
