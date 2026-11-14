//! Retire a fork at writer dispatch and after its reply without touching a replacement helper.

use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

use crate::backend::{BackendClient, RewindForkRequest};
use crate::backend_commands::ForkContext;
use crate::claude_backend::event_test_support::{command_client, SESSION};
use crate::dispatch_guard::DispatchGuard;

const TIMEOUT: Duration = Duration::from_secs(5);

fn request() -> RewindForkRequest {
    RewindForkRequest {
        keep_turns: 0,
        drop_turns: 1,
        before_turn_id: Some("before".into()),
        before_user_item: None,
    }
}

fn peer() -> (super::ClaudeClient, crate::backend::EventReceiver) {
    let mut command = Command::new("python3");
    command.args(["-u", "-c", PEER]);
    command_client(command, 1024)
}

#[test]
fn a_claude_fork_retired_at_the_writer_never_reaches_native_stdin() {
    let (client, _events) = peer();
    let checks = Arc::new(AtomicUsize::new(0));
    let observed = checks.clone();
    let context = ForkContext {
        operation: None,
        // Entry, worker target and enqueue checks pass; writer dequeue retires it.
        liveness: DispatchGuard::new(move || observed.fetch_add(1, Ordering::SeqCst) < 3),
    };
    client
        .capture_commands()
        .expect("captured helper")
        .fork_with_context(SESSION, request(), TIMEOUT, context)
        .expect_err("fork wrote no bytes");
    assert_eq!(checks.load(Ordering::SeqCst), 4);
    assert_eq!(
        client
            .helper_call("inspect", json!({}), TIMEOUT)
            .expect("healthy helper"),
        json!(["inspect"])
    );
}

#[test]
fn a_written_fork_retired_during_its_reply_wait_cannot_launch_a_replacement() {
    let (client, events) = peer();
    let live = Arc::new(AtomicBool::new(true));
    let context = ForkContext {
        operation: None,
        liveness: DispatchGuard::from_flag(live.clone()),
    };
    let commands = client.capture_commands().expect("captured helper");
    commands
        .fork_with_context(SESSION, request(), TIMEOUT, context)
        .expect("first write acknowledged");
    live.store(false, Ordering::SeqCst);
    client
        .helper_call("release", json!({}), TIMEOUT)
        .expect("release native reply");
    let failure: Value = loop {
        let line = events.recv_timeout(TIMEOUT).expect("retired fork failure");
        client.acknowledge_event(line.len());
        let value: Value = serde_json::from_str(&line).expect("fact");
        assert_ne!(value["method"], "claude/sessionChanged");
        if value["method"] == "claude/forkFailed" {
            break value;
        }
    };
    assert!(failure["params"]["message"]
        .as_str()
        .expect("diagnostic")
        .contains("retired"));
    assert!(
        commands.is_current(),
        "no replacement or healthy-helper shutdown"
    );
    assert_eq!(
        client
            .helper_call("inspect", json!({}), TIMEOUT)
            .expect("log"),
        json!(["sessions.forkBefore", "release", "inspect"])
    );
}

const PEER: &str = r#"
import json,sys
log=[]; held=None
def reply(r,result): print(json.dumps({'v':1,'type':'reply','id':r['id'],'ok':True,'result':result}),flush=True)
for line in sys.stdin:
 r=json.loads(line); m=r['method']; log.append(m)
 if m=='sessions.forkBefore': held=r; continue
 if m=='release' and held is not None:
  reply(held,{'sessionId':'00000000-0000-4000-8000-000000000002','fresh':True}); held=None
 reply(r,log if m=='inspect' else {})
 if m=='shutdown': break
"#;
