//! Prove a slow exact-target stop cannot kill unrelated native work or deliver a stale reply.

use std::process::Command;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::backend::BackendClient;
use crate::protocol::AppServerClient;
use crate::protocol_state::{request_key, Target};

#[test]
fn targeted_stop_timeout_keeps_native_progress_and_reads_alive_without_retry_or_late_reply() {
    // protocol_transport_tests::a_short_queue_read_deadline_discards_late_reply_without_stopping_native_work exercises the deadline thread (timeoutMs: 40).
    let mut client = peer();
    let events = client.take_events_rx().expect("events");
    let target = json!({"threadId":"child","turnId":"exact-turn"});
    client
        .request_control_with_context("turn/interrupt", target.clone(), json!({"timeoutMs":15000}))
        .expect("stop request");
    let receipt = events
        .recv_timeout(Duration::from_secs(30))
        .expect("native read stop");
    let receipt: Value = serde_json::from_str(&receipt).expect("receipt JSON");
    assert_eq!(receipt["params"]["delta"], "Root still running");
    let key = request_key(&receipt["params"]["requestId"]).expect("received request ID");
    let mut pending = client
        .state
        .take(&key)
        .expect("stop still awaits native reply");
    let Target::Event { deadline, .. } = &mut pending else {
        panic!("async stop target");
    };
    *deadline = Instant::now();
    client
        .state
        .insert(key, pending)
        .expect("restore same pending request");
    client.state.expire();
    let error = events
        .recv_timeout(Duration::from_secs(30))
        .expect("explicitly expired stop");
    let error: Value = serde_json::from_str(&error).expect("timeout JSON");
    assert_eq!(error["params"]["requestParams"], target);
    assert!(error["params"]["response"]["error"]["message"]
        .as_str()
        .expect("timeout reason")
        .contains("stop not confirmed"));
    // The peer sends the late stop reply before answering this read. The read's
    // completion proves the reader consumed the late reply without forwarding it.
    let reply = client
        .call("model/list", json!({}), Duration::from_secs(30))
        .expect("healthy read");
    let reply: Value = serde_json::from_str(&reply).expect("reply");
    assert_eq!(reply["result"]["stops"], json!([target]));
    assert!(
        events.try_recv().is_err(),
        "late stop reply is discarded, never retried"
    );
}

fn peer() -> AppServerClient {
    let script = r#"import sys,json
pending=[]
stops=[]
for line in sys.stdin:
 request=json.loads(line)
 if request['method'] in ('turn/interrupt','thread/backgroundTerminals/terminate'):
  stops.append(request['params'])
  pending.append(request)
  print(json.dumps({'method':'item/agentMessage/delta','params':{'threadId':'root','delta':'Root still running','requestId':request['id']}}),flush=True)
 else:
  for old in pending: print(json.dumps({'id':old['id'],'result':{}}),flush=True)
  pending=[]
  print(json.dumps({'id':request['id'],'result':{'stops':stops}}),flush=True)
"#;
    let mut command = Command::new("python3");
    command.args(["-u", "-c", script]);
    AppServerClient::start_command(command).expect("native pipe fixture")
}
