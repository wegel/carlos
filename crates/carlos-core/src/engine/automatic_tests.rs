//! Keep reads and shutdown live while a prompt file waits, and reject its reply after reset.
#![cfg(unix)]

use std::ffi::CString;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use carlos_proto::submissions::{ClientOperation, OperationState};
use serde_json::json;

use super::{EngineCommand, EngineHandle, EngineReply};
use crate::backend::BackendClient;
use crate::backend_commands::CommandHandle;
use crate::session::state::SessionState;

const TIMEOUT: Duration = Duration::from_secs(5);

struct Fixture {
    _root: tempfile::TempDir,
    engine: EngineHandle,
    notices: mpsc::Receiver<()>,
    native: CommandHandle,
    opened: mpsc::Receiver<()>,
    release: mpsc::Sender<()>,
    writer: thread::JoinHandle<()>,
}

#[test]
fn a_slow_ralph_prompt_file_cannot_enable_automatic_work_in_a_replaced_conversation() {
    let fixture = fixture();
    start_read(&fixture);
    fixture
        .native
        .request_native_control("probe", json!({}))
        .expect("native reset while file read waits");
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let EngineReply::Snapshot(snapshot) =
            request(&fixture.engine, EngineCommand::Snapshot { limit: 100 })
        else {
            panic!("responsive snapshot read");
        };
        assert!(!snapshot.info.ralph);
        if snapshot.entries.is_empty() {
            break;
        }
        fixture
            .notices
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("owner applies native reset");
    }
    fixture.release.send(()).expect("finish original file read");
    loop {
        let EngineReply::Outcomes(results) = request(&fixture.engine, EngineCommand::Outcomes)
        else {
            panic!("private results");
        };
        if results.iter().any(|result| matches!(&result.state, OperationState::Rejected { message, .. } if message.contains("session changed"))) { break; }
        fixture
            .notices
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("retired read settles");
    }
    fixture.engine.shutdown();
    fixture.engine.join().expect("stop owner");
    fixture.writer.join().expect("file fixture stops");
}

#[test]
fn shutdown_stops_the_native_backend_without_waiting_forever_for_a_prompt_file() {
    let fixture = fixture();
    start_read(&fixture);
    let started = Instant::now();
    fixture.engine.shutdown();
    fixture
        .engine
        .join()
        .expect("owner stops before file writer releases");
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(
        !fixture.native.is_current(),
        "native owner has already stopped"
    );
    fixture
        .release
        .send(())
        .expect("release detached disk read");
    fixture.writer.join().expect("file writer stops");
}

fn start_read(fixture: &Fixture) {
    let operation = ClientOperation {
        client: "11111111111111111111111111111111".parse().expect("client"),
        id: "1".parse().expect("counter"),
    };
    assert!(matches!(
        request(
            &fixture.engine,
            EngineCommand::SetRalph {
                operation,
                on: true
            }
        ),
        EngineReply::Operation(_)
    ));
    fixture
        .opened
        .recv_timeout(TIMEOUT)
        .expect("file reader waits on a real pipe");
}

fn request(engine: &EngineHandle, command: EngineCommand) -> EngineReply {
    engine
        .request(command)
        .expect("bounded admission")
        .recv_timeout(TIMEOUT)
        .expect("owner remains responsive")
}

fn fixture() -> Fixture {
    let root = tempfile::tempdir().expect("private native and file fixtures");
    let fifo = root.path().join("prompt.fifo");
    let c_path = CString::new(fifo.to_str().expect("fixture path")).expect("path without NUL");
    assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);
    let native_session = "00000000-0000-4000-8000-000000000001";
    let frames = vec![
        json!({"v":1,"type":"event","event":"sdk","message":{"type":"system","subtype":"init","session_id":native_session}}),
        json!({"v":1,"type":"event","event":"sdk","message":{"type":"conversation_reset","session_id":native_session,"uuid":"reset","new_conversation_id":"next"}}),
    ];
    let (client, events) = crate::claude_backend::receiver_client(frames, json!({}), 1024);
    let native = client.capture_commands().expect("native reset endpoint");
    let mut session = SessionState::new(native_session.into(), Default::default(), false);
    session.enable_claude();
    session.set_client_identity(carlos_proto::session::Backend::Claude, "local".into());
    session.append_message(crate::session::models::Role::Assistant, "before reset");
    session.configure_ralph_options(
        root.path().to_owned(),
        Some(fifo.to_str().expect("path").into()),
        None,
        None,
        None,
    );
    let (engine, notices) = EngineHandle::spawn(session, Box::new(client), events);
    let (ready, opened) = mpsc::channel();
    let (release, wait) = mpsc::channel();
    let writer = thread::spawn(move || {
        let mut file = OpenOptions::new()
            .write(true)
            .open(fifo)
            .expect("reader opened prompt pipe");
        ready.send(()).expect("both file endpoints are open");
        if wait.recv().is_ok() {
            file.write_all(b"Continue the current plan.")
                .expect("release prompt reader");
        }
    });
    Fixture {
        _root: root,
        engine,
        notices,
        native,
        opened,
        release,
        writer,
    }
}
