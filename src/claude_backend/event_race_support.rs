//! Coordinates deterministic peers and barriers around the actual reader threads.

use std::process::Command;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use super::event_queue::EventSender;
use super::event_test_support::command_client;
use super::transport::ControllerTransport;
use super::transport_test_hooks::TestHooks;
use super::ClaudeClient;
use crate::backend::{BackendClient, EventReceiver};

pub(super) const DEADLINE: Duration = Duration::from_secs(5);
pub(super) const BLOCKS: usize = 900;

pub(crate) struct Pause {
    reached: mpsc::Receiver<()>,
    release: mpsc::Sender<()>,
}

impl Pause {
    pub(crate) fn wait(&self) {
        self.reached
            .recv_timeout(DEADLINE)
            .expect("reader reached barrier");
    }

    pub(crate) fn resume(self) {
        self.release.send(()).expect("release reader");
    }
}

pub(super) fn pause_once(hooks: &TestHooks, name: &'static str) -> Pause {
    let (reached_tx, reached) = mpsc::channel();
    let (release, release_rx) = mpsc::channel();
    let once = Mutex::new(Some((reached_tx, release_rx)));
    hooks.set(name, move || {
        let barrier = once.lock().expect("barrier").take();
        if let Some((reached, release)) = barrier {
            reached.send(()).expect("report barrier");
            release
                .recv_timeout(DEADLINE)
                .expect("test releases barrier");
        }
    });
    Pause { reached, release }
}

pub(super) fn notify(hooks: &TestHooks, name: &'static str) -> mpsc::Receiver<()> {
    let (sender, receiver) = mpsc::channel();
    hooks.set(name, move || {
        let _ = sender.send(());
    });
    receiver
}

pub(super) fn peer() -> (ClaudeClient, EventReceiver) {
    let mut command = Command::new("python3");
    command.args(["-c", PEER]);
    command_client(command, 1024)
}

pub(super) fn transport(client: &ClaudeClient) -> Arc<ControllerTransport> {
    client.transport.lock().expect("transport").clone()
}

pub(super) fn receive(events: &EventSender, receiver: &EventReceiver) -> Value {
    let line = receiver.recv_timeout(DEADLINE).expect("receiver event");
    events.handled(line.len());
    serde_json::from_str(&line).expect("receiver JSON")
}

pub(super) fn through(client: &ClaudeClient, receiver: &EventReceiver, method: &str) -> Vec<Value> {
    let mut records = Vec::new();
    loop {
        let line = receiver.recv_timeout(DEADLINE).expect("receiver event");
        let lines = client.handle_event(&line).expect("Claude handler");
        client.acknowledge_event(line.len());
        let done = lines
            .iter()
            .any(|line| serde_json::from_str::<Value>(line).expect("JSON")["method"] == method);
        records.extend(
            lines
                .iter()
                .map(|line| serde_json::from_str::<Value>(line).expect("JSON")),
        );
        if done {
            return records;
        }
    }
}

pub(super) fn assert_blocks(records: &[Value]) {
    let texts: Vec<&str> = records
        .iter()
        .filter(|record| record["method"] == "item/completed")
        .map(|record| record["params"]["item"]["text"].as_str().expect("text"))
        .collect();
    let expected: Vec<String> = (0..BLOCKS).map(|index| index.to_string()).collect();
    assert_eq!(texts, expected);
}

pub(super) fn request(client: &ClaudeClient, method: &str) {
    client
        .request_native_control(method, json!({}))
        .expect("peer request");
}

const PEER: &str = r#"
import sys,json
SESSION='00000000-0000-4000-8000-000000000001'
owned='owned'
def sdk(message): return {'v':1,'type':'event','event':'sdk','message':message}
def assistant(content):
 return sdk({'type':'assistant','session_id':SESSION,'uuid':'snapshot','user_message_uuid':owned,'message':{'id':'native','role':'assistant','content':content}})
def emit(frame): print(json.dumps(frame),flush=True)
for line in sys.stdin:
 r=json.loads(line); method=r['method']; answer={}; ok=True
 if method=='prompt':
  owned=r['params']['uuid']; emit(assistant([]))
 if method=='init': emit(sdk({'type':'system','subtype':'init','session_id':SESSION}))
 if method=='reset': emit(sdk({'type':'conversation_reset','session_id':SESSION,'uuid':'reset','new_conversation_id':'next'}))
 if method=='burst':
  emit(assistant([{'type':'text','text':str(i)} for i in range(900)]))
 if method=='stderr':
  print('deterministic stderr failure',file=sys.stderr,flush=True)
 if method=='sessions.read':
  answer={'messages':[{'source':'human','message':{'type':'user','session_id':SESSION,'uuid':'history','message':{'content':'h'*150000}}}],'nextOffset':None}
 if method=='interrupt':
  frames=[{'v':1,'type':'reply','id':r['id'],'ok':False,'error':{'message':'rejected interrupt'}},sdk({'type':'result','subtype':'success','is_error':False,'user_message_uuid':owned}),sdk({'type':'system','subtype':'session_state_changed','state':'idle'})]
  sys.stdout.write(''.join(json.dumps(f)+'\n' for f in frames)); sys.stdout.flush(); continue
 if method=='ignored': emit(assistant([{'type':'text','text':'ignored after failure'}]))
 if method=='closed': emit({'v':1,'type':'event','event':'closed','reason':'start_failed'})
 if method=='exit': sys.exit(0)
 emit({'v':1,'type':'reply','id':r['id'],'ok':ok,'result':answer})
 if method=='shutdown': break
"#;
