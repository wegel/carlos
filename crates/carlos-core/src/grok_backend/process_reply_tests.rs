//! Hold the real writer so replacement and cancellation can retire an admitted answer.

use std::io::{self, Write};
use std::process::Command;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use super::{callback_frame, InputWriter, PendingWrite};
use crate::dispatch_guard::{DispatchGuard, DispatchRetired};
use crate::owned_child::OwnedChild;

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
