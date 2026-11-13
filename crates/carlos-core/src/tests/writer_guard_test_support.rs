//! Block the first native write so a second queued frame can be retired deterministically.

use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use serde_json::{json, Value};

use crate::dispatch_guard::{DispatchGuard, DispatchRetired};

const TIMEOUT: Duration = Duration::from_secs(5);
type Writes = Arc<Mutex<Vec<Vec<u8>>>>;

pub(crate) struct Sink {
    started: Option<mpsc::Sender<()>>,
    release: mpsc::Receiver<()>,
    writes: Writes,
}

pub(crate) struct Proof {
    started: mpsc::Receiver<()>,
    release: mpsc::Sender<()>,
    writes: Writes,
}

impl Sink {
    pub(crate) fn new() -> (Self, Proof) {
        let (started, ready) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let writes = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                started: Some(started),
                release: released,
                writes: writes.clone(),
            },
            Proof {
                started: ready,
                release,
                writes,
            },
        )
    }
}

impl Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Some(started) = self.started.take() {
            started.send(()).expect("first write entered");
            self.release
                .recv_timeout(TIMEOUT)
                .expect("release first write");
        }
        self.writes
            .lock()
            .expect("native write log")
            .push(bytes.to_vec());
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Proof {
    pub(crate) fn run(
        self,
        mut enqueue: impl FnMut(&Value, DispatchGuard) -> Result<mpsc::Receiver<Result<()>>>,
    ) {
        let first = enqueue(&json!({"method":"first"}), DispatchGuard::default())
            .expect("admit first frame");
        self.started
            .recv_timeout(TIMEOUT)
            .expect("first native write waits at barrier");
        let live = Arc::new(AtomicBool::new(true));
        let retired = enqueue(
            &json!({"method":"stale"}),
            DispatchGuard::from_flag(live.clone()),
        )
        .expect("admit second frame before retirement");
        live.store(false, Ordering::SeqCst);
        self.release.send(()).expect("release native writer");
        first
            .recv_timeout(TIMEOUT)
            .expect("first result")
            .expect("first frame written");
        let failure = retired
            .recv_timeout(TIMEOUT)
            .expect("retired result")
            .expect_err("queued stale frame must not write");
        assert!(
            failure.is::<DispatchRetired>(),
            "retirement must stay distinguishable from an uncertain OS write"
        );
        enqueue(&json!({"method":"healthy"}), DispatchGuard::default())
            .expect("healthy connection remains usable")
            .recv_timeout(TIMEOUT)
            .expect("third result")
            .expect("third frame written");
        let writes = self.writes.lock().expect("write log");
        assert_eq!(
            writes.len(),
            2,
            "only the first and healthy frames reach the native stream"
        );
        assert!(writes
            .iter()
            .all(|bytes| !String::from_utf8_lossy(bytes).contains("stale")));
    }
}
