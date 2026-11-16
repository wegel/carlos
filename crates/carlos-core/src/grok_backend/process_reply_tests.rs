//! Hold the real writer so replacement and cancellation can retire an admitted answer.

use std::io::{self, Write};
use std::process::Command;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use super::{callback_frame, InputWriter, PendingWrite};
use crate::dispatch_guard::{DispatchGuard, DispatchRetired};
use crate::owned_child::OwnedChild;
use crate::protocol_events::EventQueue;

use super::super::process_client::{cancel_agent_requests, receive_agent_request};

const TIMEOUT: Duration = Duration::from_secs(5);
type Writes = Arc<Mutex<Vec<Value>>>;

struct Sink {
    entered: Option<mpsc::Sender<()>>,
    release: mpsc::Receiver<()>,
    writes: Writes,
}

struct Fixture {
    writer: InputWriter,
    release: mpsc::Sender<()>,
    writes: Writes,
    first: PendingWrite,
    child: Arc<Mutex<OwnedChild>>,
}

impl Fixture {
    fn new() -> Self {
        let child = Arc::new(Mutex::new(
            OwnedChild::spawn(Command::new("sleep").arg("30")).expect("owned fixture"),
        ));
        let (entered, started) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let writes = Writes::default();
        let sink = Sink {
            entered: Some(entered),
            release: released,
            writes: writes.clone(),
        };
        let writer = InputWriter::start(sink, child.clone());
        let first = writer
            .enqueue_timed(&json!({"method":"held"}), DispatchGuard::default())
            .expect("first admitted");
        started
            .recv_timeout(TIMEOUT)
            .expect("native writer reached barrier");
        Self {
            writer,
            release,
            writes,
            first,
            child,
        }
    }

    fn queue_answer(&self, token: &str) -> PendingWrite {
        let callback = self
            .writer
            .callbacks
            .get(&json!(token))
            .expect("live callback");
        self.writer
            .enqueue_frames(
                vec![callback_frame(
                    callback,
                    json!({"outcome":{"outcome":"selected","optionId":"chosen"}}),
                    false,
                )],
                DispatchGuard::default(),
                false,
            )
            .expect("answer admitted")
    }

    fn release(&self) {
        self.release.send(()).expect("release writer");
        self.first
            .0
            .recv_timeout(TIMEOUT)
            .expect("first completion")
            .expect("first written");
    }

    fn written(&self) -> Vec<Value> {
        self.writes.lock().expect("captured native frames").clone()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.writer.close();
        self.child
            .lock()
            .expect("owned child")
            .wait()
            .expect("reap fixture");
    }
}

impl Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Some(entered) = self.entered.take() {
            entered.send(()).expect("writer entered");
            self.release
                .recv_timeout(TIMEOUT)
                .expect("release native writer");
        }
        self.writes
            .lock()
            .expect("native frames")
            .push(serde_json::from_slice(bytes).expect("whole native frame"));
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn a_reused_native_id_retires_an_answer_already_admitted_to_the_real_writer() {
    let fixture = Fixture::new();
    let callbacks = fixture.writer.callbacks();
    let request = json!({"id":"7","params":{"sessionId":"root","command":"old"}});
    let old = callbacks
        .register(&request, 100, 1)
        .expect("original request");
    let answer = fixture.queue_answer(&old.token);
    let mut replacement = request;
    replacement["params"]["command"] = json!("different");
    let fresh = callbacks
        .register(&replacement, 100, 1)
        .expect("replacement request");
    let chosen = fixture.queue_answer(&fresh.token);
    fixture.release();
    let error = answer
        .0
        .recv_timeout(TIMEOUT)
        .expect("old completion")
        .expect_err("old answer retired");
    assert!(error.is::<DispatchRetired>());
    chosen
        .0
        .recv_timeout(TIMEOUT)
        .expect("replacement completion")
        .expect("replacement written");
    let frames = fixture.written();
    assert_eq!(frames.len(), 2);
    assert_eq!(
        frames[1],
        json!({"jsonrpc":"2.0","id":"7","result":{"outcome":{"outcome":"selected","optionId":"chosen"}}})
    );
}

#[test]
fn cancellation_replaces_an_admitted_allow_with_exactly_one_native_cancel() {
    let fixture = Fixture::new();
    let callbacks = fixture.writer.callbacks();
    let original = callbacks
        .register(&json!({"id":-7,"params":{"sessionId":"root"}}), 100, 1)
        .expect("request");
    let grant = fixture.queue_answer(&original.token);
    fixture
        .writer
        .cancel_callbacks(Some("root"), Some(1))
        .1
        .expect("cancel admitted");
    fixture.release();
    let error = grant
        .0
        .recv_timeout(TIMEOUT)
        .expect("grant completion")
        .expect_err("queued grant retired");
    assert!(error.is::<DispatchRetired>());
    fixture
        .writer
        .write(&json!({"method":"healthy"}))
        .expect("same writer still usable");
    let frames = fixture.written();
    assert_eq!(frames.len(), 3);
    assert_eq!(
        frames[1],
        json!({"jsonrpc":"2.0","id":-7,"result":{"outcome":{"outcome":"cancelled"}}})
    );
    assert!(callbacks.get(&json!(original.token)).is_err());
}

#[test]
fn reader_rejections_enqueue_without_waiting_for_the_held_native_pipe() {
    let fixture = Fixture::new();
    let (events, _received) = EventQueue::new_for("Grok");
    for (id, method, session, retired) in [
        (1, "session/request_permission", "", false),
        (2, "session/request_permission", "root", true),
        (3, "unsupported/method", "root", false),
    ] {
        let request = json!({"id":id,"method":method,"params":{"sessionId":session}});
        receive_agent_request(request, 100, 1, retired, &fixture.writer, &events)
            .expect("reader must return while the native writer is still held");
    }
    assert_eq!(fixture.written().len(), 0, "pipe remains held");
    fixture.release();
    fixture
        .writer
        .write(&json!({"method":"barrier"}))
        .expect("all replies drained");
    let frames = fixture.written();
    assert_eq!(frames[1]["id"], 1);
    assert_eq!(frames[1]["error"]["code"], -32602);
    assert_eq!(
        frames[2],
        json!({"jsonrpc":"2.0","id":2,"result":{"outcome":{"outcome":"cancelled"}}})
    );
    assert_eq!(frames[3]["id"], 3);
    assert_eq!(frames[3]["error"]["code"], -32601);
}

#[test]
fn a_full_native_writer_retires_cards_even_when_cancel_cannot_be_enqueued() {
    let fixture = Fixture::new();
    let callbacks = fixture.writer.callbacks();
    let request = callbacks
        .register(
            &json!({"id":"permission","params":{"sessionId":"root"}}),
            100,
            1,
        )
        .expect("pending permission");
    for _ in 0..8 {
        fixture
            .writer
            .enqueue(&json!({"method":"queued"}), DispatchGuard::default())
            .expect("fill native queue while writer is held");
    }
    let (events, received) = EventQueue::new_for("Grok");
    let error = cancel_agent_requests(&fixture.writer, &events, Some("root"), Some(1))
        .expect_err("cancel could not enter the full writer");
    assert!(error.to_string().contains("queue is full"));
    let event: Value =
        serde_json::from_str(&received.recv_timeout(TIMEOUT).expect("card retirement"))
            .expect("retirement frame");
    assert_eq!(
        event,
        json!({"method":"carlos/grok/callback-retired","params":{"token":request.token}})
    );
    assert!(
        callbacks.get(&json!(request.token)).is_err(),
        "no dead grant remains"
    );
    fixture.release();
}
