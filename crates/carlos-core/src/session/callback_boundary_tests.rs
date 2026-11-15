//! Retain native JSON id limits, replay windows and deferred form fences in the owner.

use serde_json::{json, Value};

use crate::backend::BackendKind;
use crate::session::actions::ServerRequestAction;
use crate::session::approval_control_test_support::Peer;
use crate::session::approval_state::ApprovalChoice;
use crate::session::state::SessionState;

fn core() -> SessionState {
    let mut core = SessionState::new("root".into(), Default::default(), false);
    core.enable_codex();
    core
}

fn receive(core: &mut SessionState, frame: Value) -> Option<ServerRequestAction> {
    core.accept_codex_callback(frame["method"].as_str().expect("method"), &frame)
        .action
}

fn question(id: Value) -> Value {
    json!({"id":id,"method":"item/tool/requestUserInput","params":{
        "threadId":"child","turnId":"turn","itemId":"item","questions":[{
        "id":"q","header":"Header","question":"Question?","options":[]}]}})
}

#[test]
fn callback_ids_keep_json_types_and_both_original_byte_bounds() {
    for (id, accepted) in [
        (json!(""), true),
        (json!(9), true),
        (json!("9"), true),
        (json!(-3), true),
        (json!(1.5), true),
        (json!(u64::MAX), true),
        (json!("a".repeat(4094)), true),
        (json!("a".repeat(4095)), false),
        (json!("\n".repeat(2047)), true),
        (json!("\n".repeat(2048)), false),
        (Value::Null, false),
        (json!(true), false),
        (json!([]), false),
        (json!({}), false),
    ] {
        let mut core = core();
        let action = receive(&mut core, question(id.clone()));
        if accepted {
            assert!(action.is_none(), "{id}");
            assert_eq!(
                core.approval()
                    .pending()
                    .expect("accepted native id")
                    .facts()
                    .request_id,
                id
            );
        } else {
            assert!(
                matches!(
                    action,
                    Some(ServerRequestAction::ReplyError { code: -32600, .. })
                ),
                "{id}"
            );
            assert!(core.approval().pending().is_none());
        }
    }
}

#[test]
fn duplicate_and_retired_checks_precede_unsupported_methods_and_keep_2048_ids() {
    let mut core = core();
    receive(&mut core, question(json!("live")));
    let id = core.approval().pending().expect("original card").entry_id();
    assert!(receive(&mut core, json!({"id":"live","method":"future/request"})).is_none());
    assert_eq!(core.approval().pending().expect("same card").entry_id(), id);
    core.retire_codex_callback(&json!("live"));
    for n in 0..2047 {
        core.retire_codex_callback(&json!(n));
    }
    core.retire_codex_callback(&json!("live"));
    assert!(receive(&mut core, json!({"id":"live","method":"future/request"})).is_none());
    core.retire_codex_callback(&json!(2047));
    assert!(matches!(
        receive(&mut core, json!({"id":"live","method":"future/request"})),
        Some(ServerRequestAction::ReplyError { code: -32601, .. })
    ));
    assert!(receive(&mut core, json!({"id":0,"method":"future/request"})).is_none());
}

fn form() -> Value {
    json!({"id":"form","method":"mcpServer/elicitation/request","params":{
        "threadId":"child","serverName":"s","mode":"form",
        "requestedSchema":{"type":"object","properties":{}}}})
}

#[test]
fn late_form_checks_cannot_send_after_candidate_revision_or_lifetime_changes() {
    for change in ["candidate", "revision", "retired", "wrong-id"] {
        let mut core = core();
        receive(&mut core, form());
        let card = core.approval().pending().expect("card").entry_id();
        core.begin_approval_answer(card);
        let candidate =
            (change != "candidate").then(|| json!({"action":"accept","content":{"x":"old"}}));
        core.seed_codex_candidate_for_test(&json!("form"), 2, candidate, Some(card), true);
        if change == "revision" {
            core.seed_codex_candidate_for_test(
                &json!("form"),
                3,
                Some(json!({"action":"accept","content":{"x":"old"}})),
                Some(card),
                true,
            );
        } else if change == "retired" {
            core.retire_codex_callback(&json!("form"));
        }
        let id = if change == "wrong-id" {
            "absent"
        } else {
            "form"
        };
        assert!(
            core.checked_codex_callback(&json!({"requestId":id,"revision":2,"ok":true}))
                .action
                .is_none(),
            "{change}"
        );
    }
}

#[test]
fn schema_preflight_does_not_consume_the_pending_candidate() {
    for preflight_ok in [false, true] {
        let mut core = core();
        let peer = Peer::new(BackendKind::Codex);
        receive(&mut core, form());
        let card = core.approval().pending().expect("form card").entry_id();
        core.answer_codex_callback(
            &peer,
            card,
            &json!("form"),
            json!({"action":"accept","content":{}}),
            ApprovalChoice::Accept,
        )
        .expect("submitted form");
        assert_eq!(
            *peer.checks.lock().expect("one validation"),
            vec![json!(["form", {"type":"object","properties":{}}, {}, 1])]
        );
        assert!(core
            .checked_codex_callback(&json!({"requestId":"form","revision":0,"ok":preflight_ok}))
            .action
            .is_none());
        let exported = core.export_approvals().remove(0);
        assert_eq!(
            exported.input_error.as_deref(),
            if preflight_ok {
                None
            } else {
                Some("Unsupported form schema")
            }
        );
        assert!(core.submitted_approval_answer(card).is_some());
        let action = core
            .checked_codex_callback(&json!({"requestId":"form","revision":1,"ok":true}))
            .action;
        assert!(
            matches!(action, Some(ServerRequestAction::ReplyNative {result, ..})
            if result == json!({"action":"accept","content":{}}))
        );
        assert!(peer
            .calls
            .lock()
            .expect("deferred native writes")
            .is_empty());
    }
}

#[test]
fn cancelling_either_numeric_or_string_callback_never_retires_the_other() {
    for cancelled in [json!(9), json!("9")] {
        let mut core = core();
        receive(&mut core, question(json!(9)));
        let numeric = core.approval().pending().expect("numeric id").entry_id();
        receive(&mut core, question(json!("9")));
        let string = core
            .approval()
            .queued(crate::session::approval_state::ApprovalQueue::Codex)[0]
            .entry_id();
        assert_ne!(numeric, string);
        core.retire_codex_callback(&cancelled);
        let remaining = if cancelled.is_number() {
            json!("9")
        } else {
            json!(9)
        };
        assert!(core.codex_callbacks().get(&cancelled).is_none());
        assert_eq!(core.codex_callbacks().len(), 1);
        assert_eq!(
            core.approval()
                .pending()
                .expect("surviving card")
                .facts()
                .request_id,
            remaining
        );
        assert_eq!(core.export_approvals().len(), 1);
    }
}
