//! Exercise per-field settings acknowledgements against real helper pipes and retired roots.

use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use serde_json::json;

use crate::backend::BackendClient;
use crate::backend_commands::{SettingsFailure, SettingsPatch};
use crate::claude_backend::event_test_support::command_client;
use crate::dispatch_guard::DispatchGuard;

const TIMEOUT: Duration = Duration::from_secs(3);

fn peer(mode: &str) -> crate::claude_backend::ClaudeClient {
    let mut command = Command::new("python3");
    command.args(["-u", "-c", PEER, mode]);
    command_client(command, 1024).0
}

fn patch() -> SettingsPatch {
    SettingsPatch {
        model: Some(Some("chosen".into())),
        effort: Some(Some("high".into())),
    }
}

#[test]
fn claude_keeps_a_confirmed_model_after_effort_rejection() {
    let client = peer("reject");
    let result = client
        .capture_commands()
        .expect("captured")
        .change_settings(patch());
    assert_eq!(result.accepted.model, Some(Some("chosen".into())));
    assert_eq!(result.accepted.effort, None);
    assert!(matches!(result.failure, Some(SettingsFailure::Rejected(_))));
    let cached = client.runtime_settings.lock().expect("cached choices");
    assert_eq!(cached.model.as_deref(), Some("chosen"));
    assert_eq!(cached.effort, None);
}

#[test]
fn claude_never_claims_an_unconfirmed_malformed_field() {
    let client = peer("malformed");
    let result = client
        .capture_commands()
        .expect("captured")
        .change_settings(patch());
    assert!(matches!(
        result.failure,
        Some(SettingsFailure::Uncertain(_))
    ));
    assert_eq!(result.accepted.model, Some(Some("chosen".into())));
    assert_eq!(result.accepted.effort, None);
}

#[test]
fn claude_retired_permission_helper_and_root_never_write_settings() {
    for retired in 0..3 {
        let client = peer("normal");
        let live = Arc::new(AtomicBool::new(true));
        let commands = client
            .capture_commands()
            .expect("captured")
            .guarded(DispatchGuard::from_flag(live.clone()))
            .expect("guarded");
        let replacement = (retired == 2).then(|| peer("normal"));
        if let Some(replacement) = &replacement {
            *client.transport.lock().expect("owner") =
                replacement.transport.lock().expect("replacement").clone();
        } else if retired == 1 {
            client
                .transport
                .lock()
                .expect("transport")
                .conversation
                .lock()
                .expect("root")
                .epoch += 1;
        } else {
            live.store(false, Ordering::SeqCst);
        }
        let result = commands.change_settings(patch());
        assert_eq!(result.accepted, SettingsPatch::default());
        assert!(result.failure.is_some());
        assert_eq!(
            client
                .helper_call("inspect", json!({}), TIMEOUT)
                .expect("native log"),
            json!(["inspect"])
        );
    }
}

#[test]
fn claude_reply_after_root_reset_cannot_update_cached_choices_or_send_effort() {
    let client = peer("hold");
    let commands = client.capture_commands().expect("captured");
    let worker = thread::spawn(move || commands.change_settings(patch()));
    let transport = client.transport.lock().expect("transport").clone();
    loop {
        let log = transport
            .call("inspect", json!({}), TIMEOUT)
            .expect("native log");
        if log.as_array().expect("array").contains(&json!("model.set")) {
            break;
        }
    }
    transport.conversation.lock().expect("root").epoch += 1;
    transport
        .call("release", json!({}), TIMEOUT)
        .expect("late native reply");
    let result = worker.join().expect("worker");
    assert_eq!(result.accepted, SettingsPatch::default());
    assert!(matches!(
        result.failure,
        Some(SettingsFailure::Uncertain(_))
    ));
    assert_eq!(client.runtime_settings.lock().expect("cached").model, None);
    let log = transport
        .call("inspect", json!({}), TIMEOUT)
        .expect("native log");
    assert!(!log
        .as_array()
        .expect("array")
        .contains(&json!("effort.set")));
}

const PEER: &str = r#"
import json,sys
mode=sys.argv[1]; log=[]; held=None
def reply(r,**fields): print(json.dumps({'v':1,'type':'reply','id':r['id'],**fields}),flush=True)
for line in sys.stdin:
 r=json.loads(line); m=r['method']; log.append(m)
 if m=='model.set' and mode=='hold': held=r; continue
 if m=='release' and held: reply(held,ok=True,result={}); held=None
 if m=='effort.set' and mode=='reject': reply(r,ok=False,error={'message':'effort rejected'}); continue
 if m=='effort.set' and mode=='malformed': reply(r,result={}); continue
 reply(r,ok=True,result=log if m=='inspect' else {})
 if m=='shutdown': break
"#;
