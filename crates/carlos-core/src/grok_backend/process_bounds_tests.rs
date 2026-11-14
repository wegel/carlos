//! Keep native launch cancellation and unconsumed Grok facts bounded with actual pipe peers.
#![cfg(unix)]

use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixListener;
use std::process::{Command, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde_json::Value;

use super::process::start_with_events;
use super::translate::mark_prompt_started;
use super::GrokTranslationState;
use crate::launch_cancel::LaunchCancellation;
use crate::protocol_events::EventQueue;

const TIMEOUT: Duration = Duration::from_secs(5);

#[test]
fn cancellation_stops_a_real_grok_launch_before_initialize_replies() {
    let directory = tempfile::tempdir().expect("private cancellation fixture");
    let path = directory.path().join("started.sock");
    let listener = UnixListener::bind(&path).expect("barrier socket");
    let cancel = LaunchCancellation::default();
    let ticket = cancel.clone();
    let mut command = python(concat!(
        "import sys,json,socket\n",
        "s=socket.socket(socket.AF_UNIX);s.connect(sys.argv[1])\n",
        "r=json.loads(sys.stdin.readline());assert r['method']=='initialize'\n",
        "s.sendall(b'initialize is waiting\\n')\n",
        "for line in sys.stdin: pass\n"
    ));
    command.arg(path);
    let (events, _receiver) = EventQueue::new_waiting("Grok");
    let (finished, result) = mpsc::sync_channel(1);
    let worker = thread::spawn(move || {
        let launched = start_with_events(
            command,
            events,
            Arc::new(Mutex::new(GrokTranslationState::default())),
            &ticket,
        );
        let _ = finished.send(
            launched
                .map(|mut process| process.stop())
                .map_err(|error| error.to_string()),
        );
    });
    let (stream, _) = listener.accept().expect("real native process started");
    stream
        .set_read_timeout(Some(TIMEOUT))
        .expect("bounded barrier");
    let mut line = String::new();
    BufReader::new(stream)
        .read_line(&mut line)
        .expect("native initialize observed");
    assert_eq!(line, "initialize is waiting\n");
    cancel.cancel();
    assert!(
        result
            .recv_timeout(TIMEOUT)
            .expect("cancel must finish before the twenty-second initialization deadline")
            .is_err(),
        "cancelled launch cannot publish a ready backend"
    );
    worker.join().expect("launch worker stops");
    assert!(cancel.check().is_err());
}

#[test]
fn grok_stops_at_its_unconsumed_event_count_budget() {
    pressure(1, 5000);
}

#[test]
fn grok_stops_at_its_unconsumed_event_byte_budget() {
    pressure(65536, 300);
}

#[test]
fn grok_stops_an_oversized_stdout_frame_before_parsing_it() {
    let command = python(concat!(
        "import sys,json\n",
        "r=json.loads(sys.stdin.readline());print(json.dumps({'id':r['id'],'result':{}}),flush=True)\n",
        "sys.stdout.write('x'*(9*1024*1024));sys.stdout.flush()\n"
    ));
    let (events, receiver) = EventQueue::new_waiting("Grok");
    let mut process = start_with_events(
        command,
        events,
        Arc::new(Mutex::new(GrokTranslationState::default())),
        &LaunchCancellation::default(),
    )
    .expect("native initialized");
    let line = receiver
        .recv_timeout(TIMEOUT)
        .expect("bounded oversized-frame failure");
    let event: Value = serde_json::from_str(&line).expect("terminal fact");
    assert_eq!(event["method"], "carlos/grok/closed");
    assert!(event["params"]["message"]
        .as_str()
        .expect("reason")
        .contains("frame exceeds"));
    process.stop();
}

#[test]
fn grok_initialization_failure_keeps_stderr_bounded() {
    let command = python(concat!(
        "import sys,json\n",
        "r=json.loads(sys.stdin.readline());sys.stderr.write('x'*100000);sys.stderr.flush()\n",
        "print(json.dumps({'id':r['id'],'error':{'message':'native rejection'}}),flush=True)\n",
        "for line in sys.stdin: pass\n"
    ));
    let (events, _receiver) = EventQueue::new_waiting("Grok");
    let result = start_with_events(
        command,
        events,
        Arc::new(Mutex::new(GrokTranslationState::default())),
        &LaunchCancellation::default(),
    );
    let message = result
        .err()
        .expect("native initialization failed")
        .to_string();
    assert!(message.contains("native rejection"));
    assert!(
        message.len() < 17000,
        "retain at most sixteen KiB of native stderr"
    );
}

fn pressure(size: usize, count: usize) {
    let mut command = python(concat!(
        "import sys,json,time\n",
        "r=json.loads(sys.stdin.readline());print(json.dumps({'id':r['id'],'result':{}}),flush=True)\n",
        "frame={'method':'session/update','params':{'sessionId':'root','update':{'sessionUpdate':'agent_message_chunk','content':{'type':'text','text':'x'*int(sys.argv[1])}}}}\n",
        "for _ in range(int(sys.argv[2])): print(json.dumps(frame),flush=True)\n",
        "for line in sys.stdin: pass\n"
    ));
    command.args([size.to_string(), count.to_string()]);
    let mut state = GrokTranslationState {
        session_id: Some("root".into()),
        ..Default::default()
    };
    mark_prompt_started(&mut state);
    let (events, receiver) = EventQueue::new_waiting("Grok");
    let mut process = start_with_events(
        command,
        events,
        Arc::new(Mutex::new(state)),
        &LaunchCancellation::default(),
    )
    .expect("initialize fixed native peer");
    let mut bytes = 0usize;
    let mut frames = 0;
    loop {
        let line = receiver
            .recv_timeout(TIMEOUT)
            .expect("bounded backlog ends in a terminal fact");
        bytes += line.len();
        frames += 1;
        let event: Value = serde_json::from_str(&line).expect("normalized fact");
        if event["method"] == "carlos/grok/closed" {
            assert!(event["params"]["message"]
                .as_str()
                .expect("diagnostic")
                .contains("backlog"));
            break;
        }
    }
    assert!(
        frames <= 4097,
        "reserve one final close fact beyond the normal count"
    );
    assert!(
        bytes <= 16 * 1024 * 1024 + 65536,
        "reserve only bounded terminal bytes"
    );
    process.stop();
}

fn python(script: &str) -> Command {
    let mut command = Command::new("python3");
    command.args(["-u", "-c", script]);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}
