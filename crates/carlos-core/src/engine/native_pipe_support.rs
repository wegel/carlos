//! Drive the real actor forwarder between helper pipes and handler-owned Claude facts.

use std::process::Command;
use std::sync::mpsc;

use crate::backend::{BackendClient, EventReceiver};
use crate::claude_backend::forwarder_test_support::DEADLINE;
use crate::claude_backend::forwarder_test_support::{command_client, helper_call, submit};
use crate::claude_backend::ClaudeClient;
use serde_json::{json, Value};

use super::super::owner::Channels;
use super::Native;
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub(super) fn peer(order: &str, accepted: bool) -> (ClaudeClient, EventReceiver) {
    let mut command = Command::new("python3");
    command.args(["-c", PEER, order, if accepted { "yes" } else { "no" }]);
    command_client(command, 1024)
}

/// Own the real actor forwarder and close its bounded receiver on fixture teardown.
pub(super) struct Forwarded {
    native: Native,
    stop: Arc<AtomicBool>,
}
impl Deref for Forwarded {
    type Target = mpsc::Receiver<String>;
    fn deref(&self) -> &Self::Target {
        self.native.receiver.as_ref().expect("live forwarder")
    }
}
impl Drop for Forwarded {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.native.stop();
    }
}
pub(super) fn forward_native(receiver: EventReceiver) -> Forwarded {
    let channels = Channels {
        stop_request: Default::default(),
        stopping: Default::default(),
        ordinary: mpsc::channel().1,
        control: mpsc::channel().1,
        reads: mpsc::channel().1,
        wake: mpsc::sync_channel(1).0,
        wake_rx: mpsc::channel().1,
        notice: mpsc::sync_channel(1).0,
        stop: Arc::default(),
        changed: Arc::default(),
        goal_interest: Arc::default(),
        task_interest: Arc::default(),
    };
    let native = Native::start(receiver, &channels);
    Forwarded {
        native,
        stop: channels.stop,
    }
}

pub(super) fn ready(client: &ClaudeClient, ui: &mpsc::Receiver<String>) -> String {
    let id = submit(client);
    mark(client, "ready");
    through(client, ui, "ready");
    id
}

pub(super) fn mark(client: &ClaudeClient, marker: &str) {
    helper_call(client, "mark", json!({"marker":marker})).expect("reader barrier");
}

pub(super) fn through(
    client: &ClaudeClient,
    ui: &mpsc::Receiver<String>,
    marker: &str,
) -> Vec<Value> {
    let mut output = Vec::new();
    loop {
        let line = ui.recv_timeout(DEADLINE).expect("forwarded event");
        let value: Value = serde_json::from_str(&line).expect("JSON");
        let done = value["method"] == "claude/callbackCancelled"
            && value["params"]["callbackId"] == marker;
        output.extend(apply(client, &line));
        if done {
            return output;
        }
    }
}

pub(super) fn apply(client: &ClaudeClient, line: &str) -> Vec<Value> {
    let output = client.handle_event(line).expect("Claude handled lines");
    client.acknowledge_event(line.len());
    output
        .iter()
        .map(|line| serde_json::from_str(line).expect("JSON"))
        .collect()
}

pub(super) fn completions(output: &[Value]) -> Vec<Value> {
    output
        .iter()
        .filter(|line| line["method"] == "turn/completed")
        .map(|line| line["params"]["turn"].clone())
        .collect()
}

const PEER: &str = r#"
import sys,json
order=sys.argv[1].split(','); accepted=sys.argv[2]=='yes'; prompts=[]
def emit(f): print(json.dumps(f),flush=True)
def sdk(m): emit({'v':1,'type':'event','event':'sdk','message':m})
def bind(id): sdk({'type':'assistant','user_message_uuid':id,'message':{'role':'assistant','content':[]}})
def result(): sdk({'type':'result','subtype':'success','is_error':False,'user_message_uuid':prompts[0]})
def idle(): sdk({'type':'system','subtype':'session_state_changed','state':'idle'})
for line in sys.stdin:
 r=json.loads(line); method=r['method']
 if method=='prompt':
  prompts.append(r['params']['uuid'])
  if len(prompts)==1:
   sdk({'type':'system','subtype':'init','session_id':'00000000-0000-4000-8000-000000000001'})
   bind(prompts[0])
 if method=='mark': emit({'v':1,'type':'event','event':'callback_cancelled','callbackId':r['params']['marker']})
 if method=='queued': result(); idle()
 if method=='stderr': print('unanswered interrupt stderr failure',file=sys.stderr,flush=True)
 if method=='exit': sys.exit(0)
 if method=='interrupt':
  for event in order:
   if event=='barrier': emit({'v':1,'type':'event','event':'callback_cancelled','callbackId':'interrupt-pending'})
   if event=='result': result()
   if event=='idle': idle()
   if event=='second': bind(prompts[1])
   if event=='reset': sdk({'type':'conversation_reset','session_id':'00000000-0000-4000-8000-000000000001','uuid':'clear','new_conversation_id':'next'})
   if event=='stderr': print('unanswered interrupt stderr failure',file=sys.stderr,flush=True)
   if event=='reply': emit({'v':1,'type':'reply','id':r['id'],'ok':accepted,'result':{},'error':{'message':'rejected'}})
  continue
 emit({'v':1,'type':'reply','id':r['id'],'ok':True,'result':{}})
 if method=='shutdown': break
"#;
