//! Prove cancel reaches a real pipe before its own completion can retire permission.

use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use crate::backend::{BackendClient, EventReceiver};
use crate::dispatch_guard::{DispatchGuard, DispatchRetired};
use crate::grok_backend::receiver_test_support::command_client;
use crate::grok_backend::translate::mark_prompt_started;
use crate::grok_backend::GrokClient;

const TIMEOUT: Duration = Duration::from_secs(5);

#[test]
fn grok_writes_cancel_before_publishing_the_completion_that_retires_its_permission() {
    let (client, events) = fixture();
    let premature = Arc::new(AtomicBool::new(false));
    let observer = premature.clone();
    let events = Arc::new(Mutex::new(events));
    let permission = DispatchGuard::new(move || {
        while let Ok(line) = events.lock().expect("native events").try_recv() {
            let frame: Value = serde_json::from_str(&line).expect("native fact");
            if frame["method"] == "turn/completed" {
                observer.store(true, Ordering::SeqCst);
            }
        }
        !observer.load(Ordering::SeqCst)
    });
    let commands = client
        .capture_commands()
        .expect("native handle")
        .guarded(permission)
        .expect("guarded handle");
    commands
        .call("turn/interrupt", json!({}), TIMEOUT)
        .expect("cancel writes before turn retires");
    assert!(!premature.load(Ordering::SeqCst));
}

#[test]
fn the_actors_pending_alias_still_targets_its_prompt_after_native_text_names_the_turn() {
    let (client, _events) = fixture();
    client
        .capture_commands()
        .expect("native handle")
        .call(
            "turn/interrupt",
            json!({"turnId":crate::grok_backend::GROK_PENDING_TURN_ID}),
            TIMEOUT,
        )
        .expect("pending actor and first native text describe the same prompt");
}

#[test]
fn a_new_native_prompt_before_cancel_enqueue_retires_only_that_control() {
    let (client, _events) = fixture();
    let checks = Arc::new(AtomicUsize::new(0));
    let translation = client.commands.translation.clone();
    let permission = DispatchGuard::new(move || {
        if checks.fetch_add(1, Ordering::SeqCst) == 1 {
            mark_prompt_started(&mut translation.lock().expect("native prompt changes"));
        }
        true
    });
    let commands = client
        .capture_commands()
        .expect("native handle")
        .guarded(permission)
        .expect("guarded handle");
    let error = commands
        .call("turn/interrupt", json!({}), TIMEOUT)
        .expect_err("new native prompt fences old stop");
    assert!(error.is::<DispatchRetired>(), "{error}");
    assert!(
        commands.is_current(),
        "retired stop keeps the native connection healthy"
    );
}

fn fixture() -> (GrokClient, EventReceiver) {
    fixture_with_peer(PEER)
}

fn fixture_with_peer(peer: &str) -> (GrokClient, EventReceiver) {
    let mut command = Command::new("python3");
    command
        .args(["-u", "-c", peer])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let (client, events) = command_client(command);
    loop {
        let line = events.recv_timeout(TIMEOUT).expect("native ready text");
        client.acknowledge_event(line.len());
        let value: Value = serde_json::from_str(&line).expect("native fact");
        if value["method"] == "item/agentMessage/delta" {
            return (client, events);
        }
    }
}

#[test]
fn a_broken_cancel_pipe_still_retires_the_original_permission_card() {
    let peer = PEER.replace("for line in sys.stdin:", concat!(
        "import os,time\n",
        "os.close(0)\n",
        "print(json.dumps({'id':'permission','method':'session/request_permission','params':{'sessionId':'root'}}),flush=True)\n",
        "time.sleep(30)\n",
        "for line in sys.stdin:",
    ));
    let (client, events) = fixture_with_peer(&peer);
    let request: Value = loop {
        let line = events
            .recv_timeout(TIMEOUT)
            .expect("permission before broken cancel");
        client.acknowledge_event(line.len());
        let value: Value = serde_json::from_str(&line).expect("native fact");
        if value["method"] == "carlos/grok/request" {
            break value;
        }
    };
    let error = client
        .commands
        .interrupt_session("root", None, DispatchGuard::default())
        .expect_err("native stdin is closed");
    assert!(error.to_string().contains("write Grok stdin"), "{error}");
    let retired: Value = serde_json::from_str(&events.recv_timeout(TIMEOUT).expect("retired card"))
        .expect("retirement fact");
    assert_eq!(retired["method"], "carlos/grok/callback-retired");
    assert_eq!(retired["params"]["token"], request["params"]["token"]);
}

const PEER: &str = r#"
import json,sys
r=json.loads(sys.stdin.readline())
print(json.dumps({'id':r['id'],'result':{}}),flush=True)
print(json.dumps({'method':'session/update','params':{'sessionId':'root','update':{'sessionUpdate':'agent_message_chunk','content':{'type':'text','text':'ready'}}}}),flush=True)
for line in sys.stdin:
 r=json.loads(line)
 if 'id' in r: print(json.dumps({'id':r['id'],'result':{}}),flush=True)
"#;
