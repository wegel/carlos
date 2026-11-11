//! Native Codex callback IDs, drafts, focus and completion remain independent.

use crossterm::event::Event;
use serde_json::{json, Value};

use super::codex_question_support::{app, field_text, question, receive, Peer};
use crate::app::codex_callbacks::{edited, hide_nonblocking, next};
use crate::app::codex_reply::respond;
use crate::app::input_events::handle_terminal_event;
use crate::app::notifications::ServerRequestAction;
use crate::app::state::ApprovalChoice;
use crate::app::TerminalSize;

#[test]
fn exact_typed_ids_and_native_question_keys_survive_concurrent_child_requests() {
    let mut app = app();
    let peer = Peer::default();
    receive(&mut app, question(json!(9), "child-a", true));
    receive(&mut app, question(json!("9"), "child-b", true));
    app.codex
        .callbacks
        .get_mut(&json!(9))
        .expect("numeric callback")
        .draft
        .fields[0]
        .editor
        .insert_str("日本語🦀");
    app.codex
        .callbacks
        .get_mut(&json!("9"))
        .expect("string callback")
        .draft
        .fields[0]
        .editor
        .insert_str("other draft");
    respond(&peer, &mut app, ApprovalChoice::Accept);
    assert_eq!(
        peer.replies.borrow()[0],
        (
            json!(9),
            json!({"answers":{"native-q-7":{"answers":["user_note: 日本語🦀"]}}})
        )
    );
    assert_eq!(field_text(&app, json!("9")), "other draft");
    assert_eq!(
        app.approval
            .pending
            .as_ref()
            .expect("remaining callback")
            .request_id,
        json!("9")
    );
    assert!(peer.controls.borrow().is_empty());
}

#[test]
fn nonblocking_focus_and_drafts_survive_a_later_blocking_request() {
    let mut app = app();
    let peer = Peer::default();
    let size = TerminalSize {
        width: 100,
        height: 32,
    };
    receive(&mut app, question(json!("hidden"), "child-a", false));
    assert!(!crate::app::native_questions::modal(&app));
    handle_terminal_event(&peer, &mut app, Event::Paste("main draft".into()), size);
    assert_eq!(app.input_text(), "main draft");
    next(&mut app);
    handle_terminal_event(&peer, &mut app, Event::Paste("question draft".into()), size);
    assert!(hide_nonblocking(&mut app));
    receive(&mut app, question(json!("blocking"), "child-b", true));
    assert!(crate::app::native_questions::modal(&app));
    assert_eq!(
        app.approval.pending.as_ref().expect("blocking").request_id,
        "blocking"
    );
    respond(&peer, &mut app, ApprovalChoice::Decline);
    assert!(!crate::app::native_questions::modal(&app));
    assert_eq!(app.input_text(), "main draft");
    next(&mut app);
    assert_eq!(field_text(&app, json!("hidden")), "question draft");
}

#[test]
fn escape_promotes_a_queued_blocking_request_instead_of_hiding_it() {
    let mut app = app();
    receive(&mut app, question(json!("a"), "child-a", false));
    next(&mut app);
    receive(&mut app, question(json!("b"), "child-b", true));
    assert!(hide_nonblocking(&mut app));
    assert!(crate::app::native_questions::modal(&app));
    assert_eq!(
        app.approval.pending.as_ref().expect("blocking").request_id,
        "b"
    );
}

#[test]
fn exact_resolution_dedup_and_root_completion_do_not_erase_child_callbacks() {
    let mut app = app();
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
    assert_eq!(app.codex.callbacks.entries.len(), 2);
    receive(
        &mut app,
        json!({"method":"serverRequest/resolved","params":{"threadId":"wrong","requestId":9}}),
    );
    assert_eq!(app.codex.callbacks.entries.len(), 2);
    receive(
        &mut app,
        json!({"method":"serverRequest/resolved","params":{"threadId":"child","requestId":9}}),
    );
    receive(&mut app, question(json!(9), "child", false));
    assert_eq!(
        app.codex.callbacks.entries.len(),
        1,
        "resolved callback cannot resurrect"
    );
    assert!(app.codex.callbacks.get(&json!("9")).is_some());
}

#[test]
fn question_skip_and_cancel_use_distinct_native_paths_and_the_exact_child_turn() {
    let mut app = app();
    let peer = Peer::default();
    app.active_turn_id = Some("root-working".into());
    receive(&mut app, question(json!("q"), "child", true));
    respond(&peer, &mut app, ApprovalChoice::Cancel);
    assert!(peer.replies.borrow().is_empty());
    assert_eq!(
        peer.controls.borrow()[0],
        (
            "turn/interrupt".into(),
            json!({"threadId":"child","turnId":"request-turn"})
        )
    );
    assert!(app.codex.callbacks.get(&json!("q")).is_some());
    assert_eq!(app.active_turn_id.as_deref(), Some("root-working"));
    respond(&peer, &mut app, ApprovalChoice::Decline);
    assert_eq!(peer.replies.borrow()[0].1, json!({"answers":{}}));
}

#[test]
fn failed_native_reply_preserves_independent_question_and_main_drafts() {
    let mut app = app();
    let peer = Peer::default();
    peer.fail.set(true);
    app.set_input_text("main stays");
    receive(&mut app, question(json!("q"), "root", true));
    app.codex
        .callbacks
        .get_mut(&json!("q"))
        .expect("callback")
        .draft
        .fields[0]
        .editor
        .insert_str("answer stays");
    respond(&peer, &mut app, ApprovalChoice::Accept);
    assert_eq!(field_text(&app, json!("q")), "answer stays");
    assert_eq!(app.input_text(), "main stays");
    assert!(app
        .codex
        .callbacks
        .get(&json!("q"))
        .expect("callback")
        .draft
        .error
        .is_some());
}

#[test]
fn schema_preflight_applies_after_edits_but_an_obsolete_answer_cannot_send() {
    let mut app = app();
    let mut form = question(json!("form"), "child", true);
    form["method"] = json!("mcpServer/elicitation/request");
    form["params"] = json!({"threadId":"child","turnId":null,"serverName":"server","mode":"form","message":"Answer","requestedSchema":{"type":"object","properties":{"x":{"type":"string"}}}});
    assert!(matches!(
        receive(&mut app, form),
        Some(ServerRequestAction::ValidateCodexForm { revision: 0, .. })
    ));
    edited(&mut app);
    receive(
        &mut app,
        json!({"method":"carlos/codex/formChecked","params":{"requestId":"form","revision":0,"ok":false,"error":"Unsupported schema"}}),
    );
    let entry = app.codex.callbacks.get(&json!("form")).expect("callback");
    assert!(entry.draft.notice.is_none());
    assert_eq!(
        entry.draft.unsupported.as_deref(),
        Some("Unsupported schema")
    );
    let entry = app
        .codex
        .callbacks
        .get_mut(&json!("form"))
        .expect("callback");
    entry.candidate = Some(json!({"action":"accept","content":{"x":"old"}}));
    entry.revision = 2;
    edited(&mut app);
    assert!(receive(&mut app,json!({"method":"carlos/codex/formChecked","params":{"requestId":"form","revision":2,"ok":true}})).is_none());
}

#[test]
fn unknown_methods_bad_params_and_id_bounds_return_the_correct_native_error() {
    let mut app = app();
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
    assert!(app.codex.callbacks.entries.is_empty());
    assert!(app.codex.callbacks.get(&Value::Null).is_none());
}

#[test]
fn requests_with_missing_null_or_array_params_receive_errors_before_object_routing() {
    let mut app = app();
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
    let mut app = app();
    for id in 0..64 {
        assert!(receive(&mut app, question(json!(id), "child", false)).is_none());
    }
    assert!(matches!(
        receive(&mut app, question(json!(64), "child", true)),
        Some(ServerRequestAction::ReplyError { code: -32000, .. })
    ));
    assert_eq!(app.codex.callbacks.entries.len(), 64);
    crate::app::codex_callbacks::reset(&mut app);
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
        app.codex.callbacks.entries.len() < 64,
        "byte cap precedes frame cap"
    );
}

#[test]
fn replacing_a_session_retires_all_pending_callbacks_and_late_form_results() {
    let mut app = app();
    app.set_input_text("main draft");
    for id in 1..=3 {
        receive(&mut app, question(json!(id), "child", true));
    }
    app.set_thread_id("new-root");
    assert!(app.codex.callbacks.entries.is_empty());
    assert!(app.approval.pending.is_none());
    assert_eq!(app.input_text(), "main draft");
    receive(&mut app, question(json!(1), "child", true));
    assert!(app.approval.pending.is_none(), "old replay is retired");
    receive(
        &mut app,
        json!({"method":"serverRequest/resolved","params":{"threadId":"child","requestId":1}}),
    );
    receive(
        &mut app,
        question(json!("new-child"), "new-child-thread", false),
    );
    assert!(app.codex.callbacks.get(&json!("new-child")).is_some());
    assert!(receive(
        &mut app,
        json!({"method":"carlos/codex/formChecked","params":{"requestId":2,"revision":1,"ok":true}})
    )
    .is_none());
}

#[test]
fn managed_human_approvals_never_offer_or_send_session_decisions() {
    let mut app = app();
    let peer = Peer::default();
    receive(
        &mut app,
        json!({"id":7,"method":"item/commandExecution/requestApproval","params":{
        "threadId":"child","turnId":"t","itemId":"checkpoint","kind":"humanCommand",
        "availableDecisions":["accept","acceptForSession","decline","cancel"]}}),
    );
    assert!(
        !app.approval
            .pending
            .as_ref()
            .expect("approval")
            .can_accept_for_session
    );
    respond(&peer, &mut app, ApprovalChoice::AcceptForSession);
    assert!(peer.replies.borrow().is_empty());
    respond(&peer, &mut app, ApprovalChoice::Accept);
    assert_eq!(peer.replies.borrow()[0].1, json!({"decision":"accept"}));
}

#[test]
fn secret_review_is_redacted_while_the_hidden_main_composer_remains_ordinary() {
    let mut app = app();
    let mut request = question(json!("secret"), "child", false);
    request["params"]["questions"][0]["isSecret"] = json!(true);
    receive(&mut app, request);
    next(&mut app);
    let entry = app
        .codex
        .callbacks
        .get_mut(&json!("secret"))
        .expect("secret");
    entry.draft.review = true;
    assert!(crate::app::native_questions::secret_active(&app));
    hide_nonblocking(&mut app);
    assert!(!crate::app::native_questions::secret_active(&app));
}
