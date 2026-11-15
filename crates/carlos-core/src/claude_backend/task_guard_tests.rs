//! Reject a captured task stop before native stdin after a real conversation-reset frame.

use std::process::Command;
use std::time::Duration;

use serde_json::json;

use crate::backend_commands::BackendCommands;
use crate::claude_backend::event_test_support::{command_client, SESSION};

#[test]
fn a_claude_task_stop_captured_before_native_reset_never_reaches_the_writer() {
    let mut peer = Command::new("python3");
    peer.args(["-u", "-c", PEER]);
    let (client, _events) = command_client(peer, 8192);
    let captured = client.commands().expect("capture original helper and root");
    captured
        .transport
        .conversation
        .lock()
        .expect("conversation")
        .expected = Some(SESSION.into());
    client
        .helper_call("reset", json!({}), Duration::from_secs(5))
        .expect("real native reset");
    assert!(
        !captured.is_current(),
        "same helper has a new conversation epoch"
    );
    captured
        .request_native_control(
            "task.stop",
            json!({"taskId":"original-task","runId":"original-run"}),
        )
        .expect_err("captured root must reject before writing");
    let log = client
        .helper_call("inspect", json!({}), Duration::from_secs(5))
        .expect("healthy same helper");
    assert_eq!(
        log,
        json!(["reset", "inspect"]),
        "no task.stop reached native stdin"
    );
    let fresh = client.commands().expect("current reset scope");
    assert!(fresh.is_current());
    assert_eq!(
        fresh
            .helper_call("inspect", json!({}), Duration::from_secs(5))
            .expect("current helper call"),
        json!(["reset", "inspect", "inspect"])
    );
}

const PEER: &str = r#"
import json,sys
log=[]
old='00000000-0000-4000-8000-000000000001'; new='00000000-0000-4000-8000-000000000002'
def emit(value): print(json.dumps(value),flush=True)
for line in sys.stdin:
 r=json.loads(line); m=r['method']; log.append(m)
 if m=='reset':
  emit({'v':1,'type':'event','event':'sdk','message':{'type':'conversation_reset','uuid':'reset-one','session_id':old,'new_conversation_id':new}})
  emit({'v':1,'type':'event','event':'sdk','message':{'type':'system','subtype':'init','session_id':new}})
 emit({'v':1,'type':'reply','id':r['id'],'ok':True,'result':log if m=='inspect' else {}})
 if m=='shutdown': break
"#;
