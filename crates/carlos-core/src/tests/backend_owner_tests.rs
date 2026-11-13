//! Exercise a real Claude controller adapter while prompt replies remain withheld.

use std::process::Command;
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};

use super::event_test_support::{command_client, SESSION};
use super::ClaudeClient;
use crate::backend::{BackendClient, BackendKind, EventReceiver};
use crate::session::command_work::BackendWork;
use crate::session::prompt_commands::{PromptContext, PromptOptions};
use crate::session::state::SessionState;

const TIMEOUT: Duration = Duration::from_secs(5);

fn peer() -> (ClaudeClient, EventReceiver) {
    let mut command = Command::new("python3");
    command.args(["-u", "-c", PEER]);
    command_client(command, 1024)
}

fn work(state: &mut SessionState) -> BackendWork {
    state
        .prepare_turn_text(
            BackendKind::Claude,
            "held prompt".into(),
            PromptOptions {
                record_history: true,
                force_start: false,
                origin: crate::backend::PromptOrigin::Human,
            },
            &PromptContext {
                operation: None,
                claude_queue: false,
            },
        )
        .expect("prepare prompt")
        .work
        .expect("prompt work")
}

fn through(
    client: &ClaudeClient,
    receiver: &EventReceiver,
    state: &mut SessionState,
    method: &str,
) {
    loop {
        let line = receiver
            .recv_timeout(TIMEOUT)
            .expect("bounded controller fact");
        let lines = client.handle_event(&line).expect("owner normalizer");
        client.acknowledge_event(line.len());
        let mut found = false;
        for line in lines {
            let value: Value = serde_json::from_str(&line).expect("fact JSON");
            found |= value["method"] == method;
            state.handle_backend_line(&line, &state.backend_context());
        }
        if found {
            return;
        }
    }
}

#[test]
fn real_claude_owner_reads_interrupts_and_answers_before_prompt_reply() {
    let (client, events) = peer();
    let mut state = SessionState::new(SESSION.into(), Default::default(), false);
    state.enable_claude();
    let pending = work(&mut state);
    let commands = client.capture_commands().expect("captured helper");
    let worker = thread::spawn(move || pending.execute(&commands));
    through(&client, &events, &mut state, "claude/callback");
    assert_eq!(state.publish().snapshot(100).approvals.len(), 1);
    assert!(state
        .publish()
        .snapshot(100)
        .entries
        .iter()
        .any(|entry| entry.text == "fact during wait"));
    assert!(!worker.is_finished(), "native reply is still withheld");
    let commands = client.capture_commands().expect("independent controls");
    let intent = client.prepare_interrupt();
    let interrupted = commands.call("turn/interrupt", json!({}), TIMEOUT);
    client.finish_interrupt(intent, interrupted.is_ok());
    interrupted.expect("interrupt while prompt waits");
    commands
        .respond(&json!("card"), json!({"behavior":"allow"}))
        .expect("answer while prompt waits");
    let log = client
        .helper_call("inspect", json!({}), TIMEOUT)
        .expect("native log");
    assert_eq!(
        log,
        json!(["prompt", "interrupt", "callback.respond", "inspect"])
    );
    assert!(!worker.is_finished());
    client
        .helper_call("release", json!({}), TIMEOUT)
        .expect("release prompt");
    state
        .finish_backend_work(worker.join().expect("worker"))
        .expect("settle once");
}

#[test]
fn a_reset_before_real_claude_dispatch_sends_no_stale_prompt() {
    let (client, events) = peer();
    client
        .helper_call("init", json!({}), TIMEOUT)
        .expect("native init");
    let mut state = SessionState::new(SESSION.into(), Default::default(), false);
    through(&client, &events, &mut state, "thread/initialized");
    let commands = client.capture_commands().expect("old conversation");
    let pending = work(&mut state);
    client
        .helper_call("reset", json!({}), TIMEOUT)
        .expect("native reset");
    through(&client, &events, &mut state, "claude/conversationReset");
    state
        .finish_backend_work(pending.execute(&commands))
        .err()
        .expect("old command retired");
    let log = client
        .helper_call("inspect", json!({}), TIMEOUT)
        .expect("native log");
    assert_eq!(log, json!(["init", "reset", "inspect"]));
}

#[test]
fn replaced_claude_handle_never_follows_the_new_helper() {
    let (client, _events) = peer();
    let (replacement, _replacement_events) = peer();
    let old = client.capture_commands().expect("old helper");
    *client.transport.lock().expect("selector") =
        replacement.transport.lock().expect("replacement").clone();
    let error = old
        .call("thread/name/set", json!({"name":"stale"}), TIMEOUT)
        .expect_err("retired helper");
    assert!(error.to_string().contains("replaced"));
    assert_eq!(
        replacement
            .helper_call("inspect", json!({}), TIMEOUT)
            .expect("new helper log"),
        json!(["inspect"])
    );
}

const PEER: &str = r#"
import json,sys
SESSION='00000000-0000-4000-8000-000000000001'
log=[]; held=None
def emit(f): print(json.dumps(f),flush=True)
def reply(r,result): emit({'v':1,'type':'reply','id':r['id'],'ok':True,'result':result})
def sdk(m): emit({'v':1,'type':'event','event':'sdk','message':m})
for line in sys.stdin:
 r=json.loads(line); m=r['method']; log.append(m)
 if m=='prompt':
  held=r
  sdk({'type':'assistant','session_id':SESSION,'uuid':'snapshot','user_message_uuid':r['params']['uuid'],'message':{'id':'native','role':'assistant','content':[{'type':'text','text':'fact during wait'}]}})
  emit({'v':1,'type':'callback','callbackId':'card','kind':'permission','payload':{'toolName':'Read','input':{'file_path':'README.md'}}})
  continue
 if m=='callback.respond': assert r['params']=={'callbackId':'card','answer':{'behavior':'allow'}}
 if m=='init': sdk({'type':'system','subtype':'init','session_id':SESSION})
 if m=='reset': sdk({'type':'conversation_reset','session_id':SESSION,'uuid':'reset','new_conversation_id':'next'})
 if m=='release' and held is not None:
  reply(held,{}); held=None
 reply(r,log if m=='inspect' else {})
 if m=='shutdown': break
"#;
