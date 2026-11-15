//! Keep remote settings results private and retire their queued writes at the original request boundary.

use carlos_proto::submissions::{ClientOperation, OperationState};
use serde_json::{json, Value};

use super::scope::CodexScope;
use super::settings::SettingsUpdate;
use super::settings_dispatch::dispatch;
use super::settings_reply::reply;
use crate::dispatch_guard::DispatchGuard;
use crate::session::state::SessionState;

fn operation() -> ClientOperation {
    serde_json::from_value(json!({"client":"a".repeat(32),"id":"1"})).expect("operation")
}

fn begin(session: &mut SessionState) -> Vec<(Value, DispatchGuard)> {
    session.set_active_turn_id(Some("live".into()));
    session.codex_settings.deferred = Some(json!({"model":"chosen"}));
    let mut calls = Vec::new();
    assert!(dispatch(
        session,
        CodexScope::for_test(true, false, 9),
        &mut SettingsUpdate::default(),
        Some(operation()),
        |method, params, context, permission| {
            calls.push((json!({"requestMethod":method,"requestParams":params,"requestContext":context,"response":{"result":{}}}), permission));
            Ok(())
        }
    ));
    calls
}

#[test]
fn remote_settings_wait_for_both_scopes_and_duplicate_acks_cannot_reopen_the_operation() {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    let mut calls = begin(&mut session);
    let scope = CodexScope::for_test(true, false, 9);
    let mut update = SettingsUpdate::default();
    reply(&mut session, scope, &mut update, &calls[0].0);
    assert!(update.outcome.is_none());
    assert!(calls
        .iter()
        .all(|(_, permission)| permission.check().is_ok()));
    calls[1].0["response"] = json!({"error":{"code":-32602,"message":"live rejected"}});
    reply(&mut session, scope, &mut update, &calls[1].0);
    let (origin, state) = update.outcome.take().expect("one originating result");
    assert_eq!(origin, operation());
    assert!(
        matches!(state, OperationState::Accepted { notice: Some(note), .. } if note.contains("live rejected"))
    );
    assert!(calls
        .iter()
        .all(|(_, permission)| permission.check().is_err()));
    reply(&mut session, scope, &mut update, &calls[0].0);
    assert!(update.outcome.is_none());
}

#[test]
fn remote_settings_ignore_mismatched_acks_and_retiring_the_request_revokes_queued_writes() {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    let calls = begin(&mut session);
    let scope = CodexScope::for_test(true, false, 9);
    for field in ["root", "epoch", "token", "desired", "method"] {
        let mut bad = calls[0].0.clone();
        match field {
            "root" => bad["requestParams"]["threadId"] = json!("foreign"),
            "epoch" => bad["requestContext"]["epoch"] = json!(10),
            "token" => bad["requestContext"]["token"] = json!(999),
            "desired" => bad["requestParams"]["model"] = json!("different"),
            "method" => bad["requestMethod"] = json!("arbitrary/native/write"),
            _ => unreachable!("fixed negative cases"),
        }
        let mut update = SettingsUpdate::default();
        reply(&mut session, scope, &mut update, &bad);
        assert!(update.outcome.is_none(), "{field}");
        assert!(
            session.runtime_settings().current_model().is_none(),
            "{field}"
        );
    }
    session.retire_owned_settings(&operation());
    assert!(calls
        .iter()
        .all(|(_, permission)| permission.check().is_err()));
    let mut update = SettingsUpdate::default();
    reply(&mut session, scope, &mut update, &calls[0].0);
    assert!(update.outcome.is_none());
}

#[test]
fn settings_write_retires_an_older_metadata_read_and_waits_before_refreshing_it() {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.enable_codex();
    session.set_codex_runtime_refresh_for_test(true);
    let peer = crate::session::test_backend::TestBackend::default();
    assert!(super::reconnect_runtime::tick(&peer, &mut session));
    let old = peer.calls.lock().expect("metadata read")[0].clone();
    let calls = begin(&mut session);
    assert!(session.codex_connection.runtime_read.is_none());
    assert!(!super::reconnect_runtime::tick(&peer, &mut session));
    assert!(super::reconnect_runtime::reply(
        &mut session,
        &json!({
            "requestMethod":old.0,"requestParams":old.1,"requestContext":old.2,
            "response":{"result":{"thread":{"id":"root","model":"old"}}}
        })
    ));
    assert!(session.runtime_settings().current_model().is_none());
    let mut update = SettingsUpdate::default();
    let scope = CodexScope::for_test(true, false, 9);
    reply(&mut session, scope, &mut update, &calls[0].0);
    assert_eq!(
        session.runtime_settings().current_model().as_deref(),
        Some("chosen")
    );
    assert!(!super::reconnect_runtime::tick(&peer, &mut session));
    let mut live = calls[1].0.clone();
    live["response"]["result"] = json!({"status":"applied"});
    reply(&mut session, scope, &mut update, &live);
    assert!(super::reconnect_runtime::tick(&peer, &mut session));
    assert_eq!(peer.calls.lock().expect("one fresh read").len(), 2);
}
