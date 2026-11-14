//! Drain a saved Grok session while its native load waits and its producer exceeds one queue window.
#![cfg(unix)]

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use carlos_proto::commands::SendPolicy;
use carlos_proto::session::{Availability, Backend};
use carlos_proto::submissions::ClientOperation;

use super::receiver_test_support::command_client;
use super::GrokLaunchMode;
use crate::engine::{EngineCommand, EngineHandle, EngineReply};
use crate::session::state::SessionState;

const TIMEOUT: Duration = Duration::from_secs(5);

#[test]
fn owner_drains_large_grok_history_before_dispatching_a_queued_human_prompt() {
    let directory = tempfile::tempdir().expect("private native barrier");
    let path = directory.path().join("native.sock");
    let listener = UnixListener::bind(&path).expect("listen for owned fixture");
    let (client, events) = command_client(peer(path.to_str().expect("socket path")));
    let (stream, _) = listener.accept().expect("native fixture connected");
    stream
        .set_read_timeout(Some(TIMEOUT))
        .expect("bounded observation");
    let mut barrier = BufReader::new(stream);
    client
        .open_session_async(GrokLaunchMode::Resume("root".into()))
        .expect("start native load worker");
    let mut session = SessionState::new("grok-starting".into(), Default::default(), false);
    session.set_client_identity(Backend::Grok, "local".into());
    session.begin_grok_start(true);
    let (engine, notices) = EngineHandle::spawn(session, Box::new(client), events);
    assert_eq!(line(&mut barrier), "load waiting");
    let operation = ClientOperation {
        client: "11111111111111111111111111111111".parse().expect("client"),
        id: "1".parse().expect("operation"),
    };
    let admission = request(
        &engine,
        EngineCommand::Send {
            operation,
            text: "after restore".into(),
            policy: SendPolicy::Queue,
        },
    );
    assert!(matches!(admission, EngineReply::Operation(_)));
    let current = snapshot(&engine);
    assert_eq!(current.info.availability, Availability::Restoring);
    barrier
        .get_mut()
        .write_all(b"g")
        .expect("release native history");
    assert_eq!(line(&mut barrier), "prompt after restore");
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let snapshot = snapshot(&engine);
        if snapshot.entries.len() >= 2 {
            assert_eq!(snapshot.info.availability, Availability::Ready);
            assert_restored_source(&engine, &snapshot.entries[0]);
            assert_eq!(snapshot.entries[1].text, "after restore");
            break;
        }
        notices
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("owner settles send");
    }
    engine.shutdown();
    engine.join().expect("owned native process stops");
}

fn assert_restored_source(engine: &EngineHandle, entry: &carlos_proto::session::Entry) {
    assert!(entry.text.is_empty());
    assert_eq!(
        entry.source.as_ref().expect("bounded source").bytes.get(),
        6000
    );
    let EngineReply::Source(source) = request(
        engine,
        EngineCommand::Source {
            entry: entry.id.clone(),
            cursor: None,
            limit: 65536,
        },
    ) else {
        panic!("restored original source");
    };
    assert_eq!(source.text, "x".repeat(6000));
}

fn snapshot(engine: &EngineHandle) -> Box<carlos_proto::session::Snapshot> {
    let EngineReply::Snapshot(snapshot) = request(engine, EngineCommand::Snapshot { limit: 100 })
    else {
        panic!("bounded owner snapshot");
    };
    snapshot
}

fn request(engine: &EngineHandle, command: EngineCommand) -> EngineReply {
    engine
        .request(command)
        .expect("admission")
        .recv_timeout(TIMEOUT)
        .expect("responsive owner")
}

fn line(reader: &mut BufReader<std::os::unix::net::UnixStream>) -> String {
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .expect("causal native observation");
    line.trim_end().to_owned()
}

fn peer(path: &str) -> Command {
    let mut command = Command::new("python3");
    command.args(["-u", "-c", concat!(
        "import sys,json,socket\n",
        "s=socket.socket(socket.AF_UNIX);s.connect(sys.argv[1])\n",
        "for line in sys.stdin:\n",
        " r=json.loads(line);m=r['method']\n",
        " if m=='session/load':\n",
        "  s.sendall(b'load waiting\\n');assert s.recv(1)==b'g'\n",
        "  f={'method':'session/update','params':{'sessionId':'root','update':{'sessionUpdate':'agent_message_chunk','content':{'type':'text','text':'x'}}}}\n",
        "  for _ in range(6000): print(json.dumps(f),flush=True)\n",
        " if m=='session/prompt':\n",
        "  assert r['params']['prompt'][0]['text']=='after restore';s.sendall(b'prompt after restore\\n')\n",
        " print(json.dumps({'id':r['id'],'result':{'sessionId':'root','stopReason':'end_turn'}}),flush=True)\n"
    ), path]);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}
