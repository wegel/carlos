//! Preserve original native request errors, resource bounds and root replacement without editors.

use anyhow::Result;
use carlos_proto::commands::ApprovalAnswer;
use carlos_proto::session::Decision;
use serde_json::{json, Value};

use super::actions::ServerRequestAction;
use super::approval_control_test_support::Peer;
use super::approval_fact_contract_tests::assert_owner_invariants;
use super::backend_test_support::{action, context};
use super::state::SessionState;
use crate::backend::BackendKind;

fn core() -> SessionState {
    let mut core = SessionState::new("root".into(), Default::default(), false);
    core.enable_codex();
    core.enable_native_history();
    core
}

fn receive(core: &mut SessionState, frame: Value) -> Option<ServerRequestAction> {
    let update = core.handle_backend_line(&frame.to_string(), &context(core));
    let result = action(update);
    core.publish();
    assert_owner_invariants(core);
    result
}

fn question(id: Value, thread: &str, blocking: bool) -> Value {
    json!({"id":id,"method":"item/tool/requestUserInput","params":{"threadId":thread,"turnId":"request-turn","itemId":"question-tool","isBlocking":blocking,
        "questions":[{"id":"native-q-7","header":"Native question","question":"Do not use this prose as the answer key","isSecret":false,"options":null}]}})
}

#[test]
fn exact_resolution_dedup_and_root_completion_do_not_erase_child_callbacks() {
    let mut app = core();
    receive(&mut app, question(json!(9), "child", false));
    receive(&mut app, question(json!("9"), "other", true));
    receive(
        &mut app,
        json!({"method":"turn/started","params":{"threadId":"root","turn":{"id":"root-turn"}}}),
    );
    receive(
        &mut app,
        json!({"method":"turn/completed","params":{"threadId":"root","turn":{"id":"root-turn","status":"completed"}}}),
    );
    assert_eq!(app.codex_callbacks().len(), 2);
    receive(
        &mut app,
        json!({"method":"serverRequest/resolved","params":{"threadId":"wrong","requestId":9}}),
    );
    assert_eq!(app.codex_callbacks().len(), 2);
    receive(
        &mut app,
        json!({"method":"serverRequest/resolved","params":{"threadId":"child","requestId":9}}),
    );
    receive(&mut app, question(json!(9), "child", false));
    assert_eq!(
        app.codex_callbacks().len(),
        1,
        "resolved callback cannot resurrect"
    );
    assert!(app.codex_callbacks().get(&json!("9")).is_some());
}

#[test]
fn unknown_methods_bad_params_and_id_bounds_return_the_correct_native_error() {
    let mut app = core();
    let unknown = receive(
        &mut app,
        json!({"id":9,"method":"future/request","params":{}}),
    );
    assert!(matches!(
        unknown,
        Some(ServerRequestAction::ReplyError { code: -32601, .. })
    ));
    let malformed = receive(
        &mut app,
        json!({"id":10,"method":"item/tool/requestUserInput","params":{}}),
    );
    assert!(matches!(
        malformed,
        Some(ServerRequestAction::ReplyError { code: -32602, .. })
    ));
    let long = receive(&mut app, question(json!("x".repeat(4097)), "child", true));
    assert!(matches!(long, Some(ServerRequestAction::ReplyError { .. })));
    assert!(app.codex_callbacks().is_empty());
    assert!(app.codex_callbacks().get(&Value::Null).is_none());
}

#[test]
fn requests_with_missing_null_or_array_params_receive_errors_before_object_routing() {
    let mut app = core();
    for frame in [
        json!({"id":1,"method":"item/tool/requestUserInput"}),
        json!({"id":2,"method":"item/tool/requestUserInput","params":null}),
        json!({"id":3,"method":"item/tool/requestUserInput","params":[]}),
    ] {
        assert!(matches!(
            receive(&mut app, frame),
            Some(ServerRequestAction::ReplyError { code: -32602, .. })
        ));
    }
    assert!(matches!(
        receive(&mut app, json!({"id":4,"method":"future/request"})),
        Some(ServerRequestAction::ReplyError { code: -32601, .. })
    ));
}

#[test]
fn callback_count_payload_bytes_and_recent_ids_remain_bounded_after_ui_acknowledgment() {
    let mut app = core();
    for id in 0..64 {
        assert!(receive(&mut app, question(json!(id), "child", false)).is_none());
    }
    assert!(matches!(
        receive(&mut app, question(json!(64), "child", true)),
        Some(ServerRequestAction::ReplyError { code: -32000, .. })
    ));
    assert_eq!(app.codex_callbacks().len(), 64);
    for id in app.codex_callback_ids() {
        app.retire_codex_callback(&id);
    }
    let mut rejected = false;
    for id in 100..164 {
        let mut request = question(json!(id), "child", false);
        request["params"]["padding"] = json!("x".repeat(100 * 1024));
        if matches!(
            receive(&mut app, request),
            Some(ServerRequestAction::ReplyError { code: -32000, .. })
        ) {
            rejected = true;
            break;
        }
    }
    assert!(rejected);
    assert!(
        app.codex_callbacks().len() < 64,
        "byte cap precedes frame cap"
    );
}

#[test]
fn replacing_a_session_retires_all_pending_callbacks_and_late_form_results() {
    let mut app = core();
    for id in 1..=3 {
        receive(&mut app, question(json!(id), "child", true));
    }
    let _ = app.adopt_native_thread("new-root".into());
    assert!(app.codex_callbacks().is_empty());
    assert!(app.approval().pending().is_none());
    receive(&mut app, question(json!(1), "child", true));
    assert!(app.approval().pending().is_none(), "old replay is retired");
    receive(
        &mut app,
        json!({"method":"serverRequest/resolved","params":{"threadId":"child","requestId":1}}),
    );
    receive(
        &mut app,
        question(json!("new-child"), "new-child-thread", false),
    );
    assert!(app.codex_callbacks().get(&json!("new-child")).is_some());
    assert!(receive(
        &mut app,
        json!({"method":"carlos/codex/formChecked","params":{"requestId":2,"revision":1,"ok":true}})
    )
    .is_none());
}

fn answer(core: &mut SessionState, peer: &Peer, answer: ApprovalAnswer) -> Result<()> {
    let card = core.approval().pending().expect("original card").entry_id();
    let work = core.prepare_engine_answer(peer, card, answer)?;
    core.finish_engine_control(peer, work.execute(&peer.commands))?;
    Ok(())
}

#[test]
fn exact_typed_ids_and_native_question_keys_survive_concurrent_child_requests() {
    let mut core = core();
    let peer = Peer::new(BackendKind::Codex);
    receive(&mut core, question(json!(9), "child-a", true));
    receive(&mut core, question(json!("9"), "child-b", true));
    answer(
        &mut core,
        &peer,
        ApprovalAnswer::Input {
            value: json!({"fields":[{"key":"native-q-7","values":[],"text":"日本語🦀"}]}),
        },
    )
    .expect("original numeric request answered");
    assert_eq!(
        *peer.calls.lock().expect("exact native reply"),
        [(
            json!(9),
            json!({"answers":{"native-q-7":{"answers":["user_note: 日本語🦀"]}}}),
        )]
    );
    assert_eq!(
        core.approval()
            .pending()
            .expect("remaining native request")
            .facts()
            .request_id,
        json!("9")
    );
    assert!(core.codex_callbacks().get(&json!("9")).is_some());
    assert!(core.codex_callbacks().get(&json!(9)).is_none());
}

#[test]
fn managed_human_approvals_never_offer_or_send_session_decisions() {
    let mut core = core();
    let peer = Peer::new(BackendKind::Codex);
    receive(
        &mut core,
        json!({"id":7,"method":"item/commandExecution/requestApproval","params":{
        "threadId":"child","turnId":"t","itemId":"checkpoint","kind":"humanCommand",
        "availableDecisions":["accept","acceptForSession","decline","cancel"]}}),
    );
    assert!(
        !core
            .approval()
            .pending()
            .expect("managed request")
            .facts()
            .can_accept_for_session
    );
    assert!(answer(
        &mut core,
        &peer,
        ApprovalAnswer::Decision {
            decision: Decision::Session
        }
    )
    .is_err());
    assert!(peer.calls.lock().expect("no forbidden grant").is_empty());
    answer(
        &mut core,
        &peer,
        ApprovalAnswer::Decision {
            decision: Decision::Once,
        },
    )
    .expect("explicit one-time grant");
    assert_eq!(
        *peer.calls.lock().expect("one native answer"),
        [(json!(7), json!({"decision":"accept"}))]
    );
}

#[test]
fn schema_preflight_applies_after_edits_but_an_obsolete_answer_cannot_send() {
    let mut core = core();
    let mut form = question(json!("form"), "child", true);
    form["method"] = json!("mcpServer/elicitation/request");
    form["params"] = json!({"threadId":"child","turnId":null,"serverName":"server","mode":"form","message":"Answer","requestedSchema":{"type":"object","properties":{"x":{"type":"string"}}}});
    assert!(matches!(
        receive(&mut core, form),
        Some(ServerRequestAction::ValidateCodexForm { revision: 0, .. })
    ));
    core.edit_codex_callback(&json!("form"));
    assert!(receive(
        &mut core,
        json!({"method":"carlos/codex/formChecked","params":{
        "requestId":"form","revision":0,"ok":false,"error":"Unsupported schema"}})
    )
    .is_none());
    assert_eq!(
        core.export_approvals()[0].input_error.as_deref(),
        Some("Unsupported schema")
    );
    core.seed_codex_candidate_for_test(
        &json!("form"),
        2,
        Some(json!({"action":"accept","content":{"x":"old"}})),
        None,
        false,
    );
    core.edit_codex_callback(&json!("form"));
    assert!(receive(
        &mut core,
        json!({"method":"carlos/codex/formChecked","params":{
        "requestId":"form","revision":2,"ok":true}})
    )
    .is_none());
}
