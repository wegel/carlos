//! Reproduces terminal delivery waits without letting the consumer acknowledge bytes.

use std::process::Command;
use std::sync::mpsc;
use std::thread;

use serde_json::{json, Value};

use super::event_forwarder_support::forward;
use super::event_queue::MAX_EVENT_BYTES;
use super::event_race_support::{notify, pause_once, transport, DEADLINE};
use super::event_test_support::command_client;
use crate::event::UiEvent;

#[test]
fn pending_call_is_rejected_before_terminal_notices_or_bytes_are_acknowledged() {
    let (client, receiver) = peer();
    let transport = transport(&client);
    let calling = transport.clone();
    let (done_tx, done) = mpsc::channel();
    let caller = thread::spawn(move || {
        done_tx
            .send(calling.call("pending", json!({}), DEADLINE))
            .expect("call result");
    });
    let marker = receiver
        .recv_timeout(DEADLINE)
        .expect("pending call marker");
    transport.events.handled(marker.len());
    let paused = pause_once(&transport.events.hooks, "route_ready");
    let deferred = notify(&transport.events.hooks, "terminal_deferred");
    let eof = notify(&transport.events.hooks, "reader_eof");
    transport
        .events
        .send("x".repeat(MAX_EVENT_BYTES))
        .expect("fill byte budget");
    transport
        .request("die", json!({}))
        .expect("trigger helper exit");
    paused.wait();
    deferred
        .recv_timeout(DEADLINE)
        .expect("stderr waits for bytes");
    let rejected = done
        .recv_timeout(DEADLINE)
        .expect("call settled before releasing reader");
    assert_eq!(
        rejected.expect_err("helper stopped").to_string(),
        "Claude controller closed before replying"
    );
    assert!(transport.pending.lock().expect("pending").is_empty());
    paused.resume();
    eof.recv_timeout(DEADLINE)
        .expect("reader reaches EOF without acknowledgements");
    caller.join().expect("caller joined");
    let filler = receiver.recv_timeout(DEADLINE).expect("filler");
    transport.events.handled(filler.len());
    let methods: Vec<_> = (0..3)
        .map(|_| {
            let line = receiver
                .recv_timeout(DEADLINE)
                .expect("ordered terminal fact");
            transport.events.handled(line.len());
            serde_json::from_str::<Value>(&line).expect("JSON")["method"].clone()
        })
        .collect();
    assert_eq!(
        methods,
        ["claude/error", "claude/callbackCancelled", "claude/closed"]
    );
}

#[test]
fn dropping_ui_receiver_cancels_budget_through_production_forwarder() {
    let (client, receiver) = peer();
    let transport = transport(&client);
    transport
        .events
        .send("x".repeat(MAX_EVENT_BYTES))
        .expect("fill budget");
    let ui = forward(receiver);
    let UiEvent::ServerLine(line) = ui.recv_timeout(DEADLINE).expect("forwarded charged line")
    else {
        panic!("server line");
    };
    assert_eq!(line.len(), MAX_EVENT_BYTES);
    let deferred = notify(&transport.events.hooks, "terminal_deferred");
    let finished = notify(&transport.events.hooks, "finished");
    transport.request("exit", json!({})).expect("EOF");
    deferred
        .recv_timeout(DEADLINE)
        .expect("EOF delivery waits for bytes");
    drop(ui);
    finished
        .recv_timeout(DEADLINE)
        .expect("UI teardown releases final sender without an ack");
    assert_eq!(transport.events.charged_bytes(), MAX_EVENT_BYTES);
}

fn peer() -> (super::ClaudeClient, crate::backend::EventReceiver) {
    let mut command = Command::new("python3");
    command.args(["-c", PEER]);
    command_client(command, 1024)
}

const PEER: &str = r#"
import sys,json,signal
signal.signal(signal.SIGTERM, lambda *_: None)
def emit(f): print(json.dumps(dict(v=1,**f)),flush=True)
for line in sys.stdin:
 r=json.loads(line)
 if r['method']=='pending':
  emit(dict(type='event',event='callback_cancelled',callbackId='pending')); continue
 if r['method']=='stderr': print(r['params']['text'],file=sys.stderr,flush=True)
 if r['method']=='die':
  print('terminal stderr',file=sys.stderr,flush=True)
  emit(dict(type='event',event='callback_cancelled',callbackId='cancelled'))
  emit(dict(type='event',event='closed',reason='done')); break
 if r['method'] in ['exit','shutdown']: break
 emit(dict(type='reply',id=r['id'],ok=True,result={}))
"#;

#[test]
fn later_stderr_chunks_remain_visible_after_the_first_closes_the_query() {
    let (client, receiver) = peer();
    let transport = transport(&client);
    for expected in ["first stderr chunk", "second stderr chunk"] {
        transport
            .call("stderr", json!({"text":expected}), DEADLINE)
            .expect("reader serves replies after stderr");
        let line = receiver.recv_timeout(DEADLINE).expect("stderr fact");
        transport.events.handled(line.len());
        let value: Value = serde_json::from_str(&line).expect("JSON");
        assert_eq!(
            value["params"]["error"]["message"],
            format!("Claude controller: {expected}")
        );
    }
}
