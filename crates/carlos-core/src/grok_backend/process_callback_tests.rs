//! Keep reused ACP ids, cancellation and callback budgets exact before any native write.

use serde_json::json;

use super::Callbacks;

#[test]
fn integer_and_string_ids_keep_distinct_targets_and_identical_delivery_keeps_its_token() {
    let callbacks = Callbacks::default();
    let number =
        json!({"id":7,"method":"session/request_permission","params":{"sessionId":"root"}});
    let string =
        json!({"id":"7","method":"session/request_permission","params":{"sessionId":"root"}});
    let first = callbacks
        .register(&number, 100, 0)
        .expect("numeric request");
    let duplicate = callbacks.register(&number, 100, 0).expect("same delivery");
    let second = callbacks.register(&string, 100, 0).expect("string request");
    assert_eq!(first.token, duplicate.token);
    assert_ne!(first.token, second.token);
    assert_eq!(
        callbacks
            .get(&json!(first.token))
            .expect("numeric target")
            .id(),
        &json!(7)
    );
    assert_eq!(
        callbacks
            .get(&json!(second.token))
            .expect("string target")
            .id(),
        &json!("7")
    );
}

#[test]
fn a_replacement_request_retires_an_already_captured_native_target() {
    let callbacks = Callbacks::default();
    let request = json!({"id":7,"method":"session/request_permission","params":{"command":"old"}});
    let first = callbacks.register(&request, 100, 0).expect("first request");
    let captured = callbacks
        .get(&json!(first.token))
        .expect("captured old target");
    let mut replacement = request;
    replacement["params"]["command"] = json!("new");
    let second = callbacks
        .register(&replacement, 100, 0)
        .expect("replacement request");
    assert_eq!(second.retired.as_deref(), Some(first.token.as_str()));
    assert!(callbacks.dispatch(&captured, false).is_err());
    assert!(callbacks.get(&json!(first.token)).is_err());
    let fresh = callbacks.get(&json!(second.token)).expect("new target");
    assert!(callbacks.dispatch(&fresh, false).is_ok());
}

#[test]
fn cancellation_wins_before_dispatch_and_never_answers_twice() {
    let callbacks = Callbacks::default();
    let first = callbacks
        .register(&json!({"id":-7,"params":{"sessionId":"root"}}), 100, 0)
        .expect("request");
    let captured = callbacks
        .get(&json!(first.token))
        .expect("captured request");
    let cancelled = callbacks.cancel_session(Some("root"), Some(0));
    assert_eq!(cancelled.len(), 1);
    assert!(callbacks.dispatch(&captured, false).is_err());
    assert!(callbacks.dispatch(&captured, true).is_ok());
    assert!(callbacks.dispatch(&captured, true).is_err());
    callbacks.finish(captured.token());
    assert!(callbacks.get(&json!(captured.token())).is_err());
}

#[test]
fn callback_budget_rejects_overload_and_closing_retires_captured_work() {
    let callbacks = Callbacks::default();
    let first = callbacks
        .register(&json!({"id":"first"}), 16 * 1024 * 1024, 0)
        .expect("entire byte allowance");
    let captured = callbacks
        .get(&json!(first.token))
        .expect("captured request");
    assert!(callbacks.register(&json!({"id":"second"}), 1, 0).is_err());
    callbacks.close();
    assert!(callbacks.dispatch(&captured, false).is_err());
    assert!(callbacks.register(&json!({"id":"third"}), 1, 0).is_err());
}

#[test]
fn a_new_prompt_retires_identical_ids_and_in_flight_reuse_fails_closed() {
    let callbacks = Callbacks::default();
    let request = json!({"id":7,"params":{"sessionId":"root"}});
    let old = callbacks.register(&request, 100, 1).expect("old prompt");
    let captured = callbacks.get(&json!(old.token)).expect("old target");
    let next = callbacks.register(&request, 100, 2).expect("new prompt");
    assert_ne!(old.token, next.token);
    assert!(callbacks.dispatch(&captured, false).is_err());
    assert!(callbacks.cancel_session(Some("root"), Some(1)).is_empty());
    let fresh = callbacks.get(&json!(next.token)).expect("new target");
    callbacks
        .dispatch(&fresh, false)
        .expect("writer claimed original target");
    assert!(
        callbacks.register(&request, 100, 3).is_err(),
        "a reply may already be writing to the reused raw id"
    );
    assert!(callbacks.dispatch(&fresh, false).is_err());
}
