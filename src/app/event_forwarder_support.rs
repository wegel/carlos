//! Drives the production event forwarder between real helper pipes and a handler-owned client.

use std::process::Command;
use std::sync::mpsc;

use serde_json::{json, Value};

use crate::backend::{BackendClient, EventReceiver};
use crate::claude_backend::forwarder_test_support::DEADLINE;
use crate::claude_backend::forwarder_test_support::{command_client, helper_call, submit};
use crate::claude_backend::ClaudeClient;
use crate::event::UiEvent;

pub(super) fn peer(order: &str, accepted: bool) -> (ClaudeClient, EventReceiver) {
    let mut command = Command::new("python3");
    command.args(["-c", PEER, order, if accepted { "yes" } else { "no" }]);
    command_client(command, 1024)
}

pub(super) fn forward(receiver: EventReceiver) -> EventReceiver<UiEvent> {
    let (_, summaries) = mpsc::channel();
    #[cfg(feature = "dictation")]
    {
        let (_, dictation) = mpsc::channel();
        crate::event::spawn_event_forwarders_with_dictation(receiver, dictation, summaries)
    }
    #[cfg(not(feature = "dictation"))]
    crate::event::spawn_event_forwarders(receiver, summaries)
}

pub(super) fn ready(client: &ClaudeClient, ui: &mpsc::Receiver<UiEvent>) -> String {
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
    ui: &mpsc::Receiver<UiEvent>,
    marker: &str,
) -> Vec<Value> {
    let mut output = Vec::new();
    loop {
        if let UiEvent::ServerLine(line) = ui.recv_timeout(DEADLINE).expect("forwarded event") {
            let value: Value = serde_json::from_str(&line).expect("JSON");
            let done = value["method"] == "claude/callbackCancelled"
                && value["params"]["callbackId"] == marker;
            output.extend(apply(client, &line));
            if done {
                return output;
            }
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
