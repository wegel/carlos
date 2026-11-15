//! Keep explicit reopen tied to its native launch and retired helper capture.

use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use super::conversation::ConversationGuard;
use super::event_test_support::{command_client, SESSION};
use super::reopen::{record_launch, LaunchPlan};
use super::types::ClaudeLaunchMode;
use crate::backend::BackendClient;
use crate::dispatch_guard::DispatchGuard;

#[test]
fn fresh_launch_provenance_survives_native_init_and_clear_without_guessing_saved_history() {
    let mut conversation = ConversationGuard::default();
    conversation.expected = Some(SESSION.to_string());
    conversation.launch = Some(LaunchPlan {
        id: SESSION.to_string(),
        mode: ClaudeLaunchMode::New,
    });
    conversation
        .route(json!({"v":1,"type":"reply","ok":false}))
        .expect("failed start");
    assert!(matches!(
        conversation.launch.as_ref().expect("fresh intent").mode,
        ClaudeLaunchMode::New
    ));
    conversation.route(json!({"v":1,"type":"event","event":"sdk","message":{"type":"system","subtype":"init","session_id":SESSION}})).expect("native root init");
    assert!(matches!(
        conversation
            .launch
            .as_ref()
            .expect("fresh intent after init")
            .mode,
        ClaudeLaunchMode::New
    ));
    let next = "22222222-2222-4222-8222-222222222222";
    conversation.route(json!({"v":1,"type":"event","event":"sdk","message":{"type":"conversation_reset","session_id":SESSION,"uuid":"reset","new_conversation_id":"reset-key"}})).expect("reset permits one identity");
    conversation.route(json!({"v":1,"type":"event","event":"sdk","message":{"type":"system","subtype":"init","session_id":next}})).expect("native next root");
    let plan = conversation.launch.as_ref().expect("fresh reset intent");
    assert_eq!(plan.id, next);
    assert!(matches!(plan.mode, ClaudeLaunchMode::New));
}

#[test]
fn retired_reopen_permission_cannot_stop_the_captured_helper() {
    let (client, _events) = peer();
    let commands = client.capture_commands().expect("helper capture");
    let commands = commands
        .guarded(DispatchGuard::from_flag(Arc::new(AtomicBool::new(false))))
        .expect("guarded capture");
    assert!(commands.reopen_session(SESSION, None, None).is_err());
    assert_eq!(
        client
            .helper_call("inspect", json!({}), Duration::from_secs(5))
            .expect("original still alive"),
        json!(["inspect"])
    );
}

#[test]
fn reset_after_reopen_capture_cannot_stop_or_replace_the_helper() {
    let (client, _events) = peer();
    let commands = client.capture_commands().expect("helper capture");
    client
        .transport
        .lock()
        .expect("helper")
        .conversation
        .lock()
        .expect("conversation")
        .epoch += 1;
    assert!(commands.reopen_session(SESSION, None, None).is_err());
    assert_eq!(
        client
            .helper_call("inspect", json!({}), Duration::from_secs(5))
            .expect("reset helper still alive"),
        json!(["inspect"])
    );
}

fn peer() -> (super::ClaudeClient, crate::backend::EventReceiver) {
    let mut command = Command::new("python3");
    command.args(["-u", "-c", PEER]);
    let (client, events) = command_client(command, 1024);
    record_launch(
        &client.transport.lock().expect("helper"),
        &ClaudeLaunchMode::New,
        SESSION,
    )
    .expect("chosen native launch");
    (client, events)
}

const PEER: &str = r#"
import json,sys
log=[]
for line in sys.stdin:
 r=json.loads(line); log.append(r['method'])
 print(json.dumps({'v':1,'type':'reply','id':r['id'],'ok':True,'result':log if r['method']=='inspect' else {}}),flush=True)
 if r['method']=='shutdown': break
"#;
