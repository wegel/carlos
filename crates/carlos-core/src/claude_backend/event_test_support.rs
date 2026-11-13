//! Builds a receiver-level Claude client around a deterministic local controller peer.

use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use super::event_queue::EventSender;
use super::transport::ControllerTransport;
use super::ClaudeClient;
use crate::backend::{BackendClient, EventReceiver, PromptOrigin, PromptPriority};

pub(super) const SESSION: &str = "00000000-0000-4000-8000-000000000001";

/// Start a real receiver around a deterministic local peer and its scripted frames.
pub fn client(frames: Vec<Value>, result: Value, capacity: usize) -> (ClaudeClient, EventReceiver) {
    scripted(json!(frames), result, capacity)
}

pub(super) fn scripted(
    frames: Value,
    result: Value,
    capacity: usize,
) -> (ClaudeClient, EventReceiver) {
    let mut command = Command::new("python3");
    command.args(["-c", concat!(
        "import sys,json,base64\n",
        "frames=json.loads(sys.argv[1]); result=json.loads(sys.argv[2])\n",
        "last_uuid='owned'\n",
        "encoded=None\n",
        "if '_historyBytes' in result:\n",
        " encoded=json.dumps({'source':'human','message':{'type':'user','session_id':'00000000-0000-4000-8000-000000000001','uuid':'large-user','message':{'content':'x'*result['_historyBytes']}}}).encode()\n",
        "for line in sys.stdin:\n",
        " r=json.loads(line)\n",
        " last_uuid=r['params'].get('uuid',last_uuid)\n",
        " batch=frames.get(r['method'],[]) if isinstance(frames,dict) else frames if r['method'] in ['prompt','probe'] else []\n",
        " for f in batch:\n",
        "  print(json.dumps(json.loads(json.dumps(f).replace('$prompt',last_uuid))),flush=True)\n",
        " answer=result\n",
        " if encoded is not None and r['method']=='sessions.read':\n",
        "  offset=r['params'].get('chunkOffset',0); end=min(len(encoded),offset+262144)\n",
        "  answer={'chunk':{'offset':offset,'totalBytes':len(encoded),'data':base64.b64encode(encoded[offset:end]).decode()},'nextOffset':0 if end<len(encoded) else None,'chunkOffset':end if end<len(encoded) else None}\n",
        " print(json.dumps({'v':1,'type':'reply','id':r['id'],'ok':(r['method']!='interrupt' or result.get('_interruptOk',False)) and r['method'] not in result.get('_failedMethods',[]),'error':{'message':'failed interrupt'},'result':answer}),flush=True)\n",
        " if r['method']=='shutdown': break\n"
    ), &serde_json::to_string(&frames).expect("peer frames"), &result.to_string()]);
    command_client(command, capacity)
}

pub(super) fn command_client(command: Command, capacity: usize) -> (ClaudeClient, EventReceiver) {
    let (events, receiver) = EventSender::channel(capacity);
    let transport =
        ControllerTransport::start_command(command, events.clone()).expect("start peer");
    transport
        .normalizer
        .lock()
        .expect("normalizer")
        .live
        .stream_instance_seq = 17;
    let mut client = ClaudeClient {
        handler: Default::default(),
        cwd: PathBuf::from("."),
        transport: Arc::new(Mutex::new(Arc::new(transport))),
        session_id: SESSION.to_string(),
        runtime_settings: Mutex::new(Default::default()),
        events_tx: events,
        events_rx: Some(receiver),
    };
    let receiver = client.take_events_rx().expect("backend receiver");
    (client, receiver)
}

#[cfg(test)]
pub(super) fn sdk(message: Value) -> Value {
    json!({"v":1,"type":"event","event":"sdk","message":message})
}

#[cfg(test)]
pub(super) fn stream(event: Value) -> Value {
    sdk(json!({"type":"stream_event","event":event}))
}

pub(super) fn submit(client: &ClaudeClient) -> String {
    let response = client
        .submit_prompt(
            "turn/start",
            json!({"input":[{"text":"hello"}]}),
            PromptOrigin::Human,
            PromptPriority::Queue,
            Duration::from_secs(2),
        )
        .expect("submit prompt");
    serde_json::from_str::<Value>(&response).expect("response")["result"]["turn"]["id"]
        .as_str()
        .expect("prompt UUID")
        .to_string()
}

pub(super) fn records(client: &ClaudeClient, receiver: &EventReceiver) -> Vec<Value> {
    receiver
        .try_iter()
        .flat_map(|line| {
            let lines = client
                .handle_event(&line)
                .unwrap_or_else(|| vec![line.clone()]);
            client.acknowledge_event(line.len());
            lines
        })
        .map(|line| {
            let value: Value = serde_json::from_str(&line).expect("normalized JSON");
            assert_ne!(value["method"], "claude/controller");
            assert!(value["params"].get("frame").is_none());
            value
        })
        .collect()
}

#[cfg(test)]
pub(super) fn expected(method: &str, mut params: Value) -> Value {
    params["generation"] = json!(1);
    params["epoch"] = json!(0);
    json!({"method":method,"params":params})
}

#[cfg(test)]
pub(super) fn assert_eof(client: &ClaudeClient, receiver: &EventReceiver) {
    let line = receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("main also reports EOF after overflow");
    client.acknowledge_event(line.len());
    let eof: Value = serde_json::from_str(&line).expect("EOF JSON");
    assert_eq!(
        eof["params"]["error"]["message"],
        "Claude controller stdout closed unexpectedly"
    );
    assert_eq!(client.events_tx.charged_bytes(), 0);
}
