//! Prove the real ACP reader and priority controls remain live during native waits.

use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use serde_json::json;

use super::receiver_test_support::command_client;
use crate::backend::{BackendClient, EventReceiver};
use crate::session::state::SessionState;

const TIMEOUT: Duration = Duration::from_secs(5);

fn command(script: &str) -> Command {
    let mut command = Command::new("python3");
    command
        .args(["-u", "-c", script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

fn receive(events: &EventReceiver, state: &mut SessionState, text: &str) {
    loop {
        let line = events.recv_timeout(TIMEOUT).expect("bounded ACP fact");
        state.handle_backend_line(&line, &state.backend_context());
        if state
            .publish()
            .snapshot(100)
            .entries
            .iter()
            .any(|entry| entry.text.contains(text))
        {
            return;
        }
    }
}

#[test]
fn real_grok_control_and_owner_read_proceed_during_a_model_reply_wait() {
    let (client, events) = command_client(command(PEER));
    let commands = client.capture_commands().expect("capture");
    let worker = thread::spawn(move || {
        commands.call(
            "turn/start",
            json!({"model":"held","input":[{"text":"hello"}]}),
            TIMEOUT,
        )
    });
    let mut state = SessionState::new("root".into(), Default::default(), false);
    receive(&events, &mut state, "fact during wait");
    assert!(!worker.is_finished(), "set_model reply is withheld");
    client
        .capture_commands()
        .expect("control handle")
        .call("turn/interrupt", json!({}), TIMEOUT)
        .expect("control before model reply");
    // The peer releases set_model only after observing session/cancel.
    worker
        .join()
        .expect("worker")
        .expect("released model and prompt");
    assert!(client
        .capture_commands()
        .expect("handle")
        .respond(&json!("unsupported"), json!({}))
        .is_err());
}

#[test]
fn a_real_grok_handle_is_retired_even_when_rewind_keeps_its_session_id() {
    let (client, _events) = command_client(command(PEER));
    let commands = client.capture_commands().expect("old capture");
    client.store_session_id("root".into());
    assert!(commands
        .call("turn/start", json!({"input":[{"text":"stale"}]}), TIMEOUT)
        .expect_err("same id, new conversation")
        .to_string()
        .contains("changed"));
}

#[test]
fn blocked_grok_stdin_has_a_deadline_without_borrowing_the_owner() {
    let (client, events) = command_client(command(BLOCKED));
    let commands = client.capture_commands().expect("capture");
    let worker = thread::spawn(move || {
        commands.call(
            "turn/start",
            json!({"input":[{"text":"x".repeat(2 * 1024 * 1024)}]}),
            TIMEOUT,
        )
    });
    let mut state = SessionState::new("root".into(), Default::default(), false);
    receive(&events, &mut state, "ready");
    let result = worker
        .join()
        .expect("bounded writer")
        .expect_err("blocked pipe");
    assert!(result.to_string().contains("500 ms"), "{result}");
}

#[test]
fn grok_eof_rejects_a_waiting_model_change_and_keeps_the_accepted_cache() {
    let script = PEER.replace("held=r; text('fact during wait'); continue", "sys.exit(0)");
    let (client, _events) = command_client(command(&script));
    let commands = client.capture_commands().expect("capture");
    let error = commands
        .call(
            "turn/start",
            json!({"model":"held","input":[{"text":"hello"}]}),
            TIMEOUT,
        )
        .expect_err("EOF cannot accept settings");
    assert!(error.to_string().contains("stdout closed"), "{error}");
    assert!(client
        .runtime_settings
        .lock()
        .expect("settings")
        .model
        .is_none());
}

const PEER: &str = r#"
import json,sys
held=None
def emit(f): print(json.dumps(f),flush=True)
def text(s): emit({'method':'session/update','params':{'sessionId':'root','update':{'sessionUpdate':'agent_message_chunk','content':{'type':'text','text':s}}}})
for line in sys.stdin:
 r=json.loads(line); m=r.get('method')
 if m=='session/set_model':
  held=r; text('fact during wait'); continue
 if m=='session/cancel':
  text('control arrived')
  if held: emit({'id':held['id'],'result':{}}); held=None
 if 'id' in r: emit({'id':r['id'],'result':{'stopReason':'end_turn'}})
"#;

const BLOCKED: &str = r#"
import json,sys,time
r=json.loads(sys.stdin.readline())
print(json.dumps({'id':r['id'],'result':{}}),flush=True)
print(json.dumps({'method':'session/update','params':{'sessionId':'root','update':{'sessionUpdate':'agent_message_chunk','content':{'type':'text','text':'ready'}}}}),flush=True)
time.sleep(30)
"#;
