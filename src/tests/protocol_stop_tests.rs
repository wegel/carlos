//! Prove a slow exact-target stop cannot kill unrelated native work or deliver a stale reply.

use std::process::Command;
use std::time::Duration;

use serde_json::{json, Value};

use crate::backend::BackendClient;
use crate::protocol::AppServerClient;

#[test]
fn targeted_stop_timeout_keeps_native_progress_and_reads_alive_without_retry_or_late_reply() {
    let script = r#"import sys,json,time,threading
stops=[]
def late(request):
 time.sleep(.2)
 print(json.dumps({'id':request['id'],'result':{}}),flush=True)
for line in sys.stdin:
 request=json.loads(line)
 if request['method'] in ('turn/interrupt','thread/backgroundTerminals/terminate'):
  stops.append(request['params'])
  threading.Thread(target=late,args=(request,),daemon=True).start()
  print(json.dumps({'method':'item/agentMessage/delta','params':{'threadId':'root','delta':'Root still running'}}),flush=True)
 else:
  print(json.dumps({'id':request['id'],'result':{'stops':stops}}),flush=True)
"#;
    let mut command = Command::new("python3");
    command.args(["-u", "-c", script]);
    let mut client = AppServerClient::start_command(command).expect("native pipe fixture");
    let events = client.take_events_rx().expect("events");
    let target = json!({"threadId":"child","turnId":"exact-turn"});
    client
        .request_control_with_context("turn/interrupt", target.clone(), json!({"timeoutMs":40}))
        .expect("stop request");
    let mut error = false;
    let mut progress = false;
    for _ in 0..2 {
        let line = events
            .recv_timeout(Duration::from_secs(2))
            .expect("progress or timeout");
        progress |= line.contains("Root still running");
        error |= line.contains("stop not confirmed");
        client.acknowledge_event(line.len());
    }
    assert!(progress && error);
    std::thread::sleep(Duration::from_millis(250));
    let reply = client
        .call("model/list", json!({}), Duration::from_secs(2))
        .expect("healthy read");
    let reply: Value = serde_json::from_str(&reply).expect("reply");
    assert_eq!(reply["result"]["stops"], json!([target]));
    assert!(
        events.try_recv().is_err(),
        "late stop reply is discarded, never retried"
    );
}
