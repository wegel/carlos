//! Correlates versioned controller requests while a separate thread forwards native callbacks.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};

use super::conversation::ConversationGuard;
use super::event_emitter::EventEmitter;
use super::event_normalizer::EventNormalizer;
use super::event_queue::EventSender;
use super::helper::controller_executable;
use super::transport_reader::spawn_readers;
use super::transport_state::{ShutdownRequest, TransportState, SHUTDOWN_TIMEOUT};
use super::transport_workers::TransportWorkers;
use super::transport_writer::{InputWriter, OversizedFrame};

/// Matches local request IDs with waiters or UI events.
pub(super) type Pending = Arc<Mutex<HashMap<String, RequestTarget>>>;
/// Holds an optional caller-requested wire log.
pub(super) type Trace = Option<Arc<Mutex<File>>>;

/// Chooses where one controller reply should go.
pub(super) enum RequestTarget {
    Reply {
        sender: mpsc::Sender<std::result::Result<Value, String>>,
    },
    Event {
        method: String,
        context: Value,
        epoch: u64,
    },
}

/// Owns the helper process and its bounded request writer.
pub(super) struct ControllerTransport {
    writer: InputWriter,
    pub(super) workers: Arc<TransportWorkers>,
    pub(super) normalizer: Arc<Mutex<EventNormalizer>>,
    pub(super) emitter: Arc<EventEmitter>,
    #[cfg(test)]
    pub(super) events: EventSender,
    pub(super) generation: u64,
    pub(super) pending: Pending,
    pub(super) next_id: Arc<AtomicU64>,
    pub(super) state: Arc<TransportState>,
    pub(super) trace: Trace,
    pub(super) conversation: Arc<Mutex<ConversationGuard>>,
    pub(super) child: Arc<Mutex<Child>>,
}

impl ControllerTransport {
    /// Start the compiled helper without waiting for SDK startup.
    pub(super) fn start(events: EventSender) -> Result<Self> {
        Self::start_command(Command::new(controller_executable()?), events)
    }

    /// Start a supplied helper command for production or deterministic pipe tests.
    pub(super) fn start_command(mut command: Command, events: EventSender) -> Result<Self> {
        let trace = std::env::var_os("CARLOS_CLAUDE_WIRE_TRACE")
            .map(|path| OpenOptions::new().create(true).append(true).open(path))
            .transpose()?
            .map(|file| Arc::new(Mutex::new(file)));
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("start Claude SDK controller")?;
        let state = Arc::new(TransportState::default());
        let workers = Arc::new(TransportWorkers::default());
        let events = events.with_workers(workers.clone());
        let writer = InputWriter::start(
            child.stdin.take().context("missing controller stdin")?,
            trace.clone(),
            state.clone(),
            workers.clone(),
        );
        let stdout = child.stdout.take().context("missing controller stdout")?;
        let stderr = child.stderr.take().context("missing controller stderr")?;
        let normalizer = Arc::new(Mutex::new(EventNormalizer::default()));
        let child = Arc::new(Mutex::new(child));
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let emitter = Arc::new(EventEmitter::new(
            events.clone(),
            normalizer.clone(),
            child.clone(),
            state.clone(),
            pending.clone(),
        ));
        let transport = Self {
            writer,
            workers,
            normalizer,
            emitter,
            #[cfg(test)]
            events: events.clone(),
            generation: events.generation,
            pending,
            next_id: Arc::new(AtomicU64::new(1)),
            state,
            trace,
            conversation: Arc::new(Mutex::new(ConversationGuard::default())),
            child,
        };
        spawn_readers(stdout, stderr, &transport);
        Ok(transport)
    }

    /// Wait for one correlated controller reply within the caller deadline.
    pub(super) fn call(&self, method: &str, params: Value, timeout: Duration) -> Result<Value> {
        let (sender, receiver) = mpsc::channel();
        let id = self.send(method, params, RequestTarget::Reply { sender })?;
        match receiver.recv_timeout(timeout) {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(message)) => bail!(message),
            Err(_) => {
                if let Ok(mut pending) = self.pending.lock() {
                    pending.remove(&id);
                }
                self.emitter.stop();
                bail!(
                    "Claude controller did not answer {method} within {} seconds; session stopped because the request may already have been accepted",
                    timeout.as_secs()
                )
            }
        }
    }

    /// Send a command whose reply will arrive through the UI event queue.
    pub(super) fn request(&self, method: &str, params: Value) -> Result<()> {
        let epoch = self
            .conversation
            .lock()
            .map_err(|_| anyhow!("session guard lock poisoned"))?
            .epoch;
        let context = json!({"revision":params["revision"],"contextToken":params["contextToken"],"taskId":params["taskId"],"runId":params["runId"]});
        self.send(
            method,
            params,
            RequestTarget::Event {
                method: method.to_string(),
                context,
                epoch,
            },
        )
        .map(|_| ())
    }

    fn send(&self, method: &str, params: Value, target: RequestTarget) -> Result<String> {
        if self.state.is_closed() {
            bail!("Claude controller is closed");
        }
        if method == "start" {
            self.conversation
                .lock()
                .map_err(|_| anyhow!("session guard lock poisoned"))?
                .expected = params["resume"]
                .as_str()
                .or_else(|| params["sessionId"].as_str())
                .map(str::to_string);
        }
        if matches!(method, "sessions.info" | "sessions.read") {
            self.normalizer
                .lock()
                .map_err(|_| anyhow!("Claude normalizer lock poisoned"))?
                .history
                .session = params["sessionId"].as_str().map(str::to_string);
        }
        let id = format!("rust-{}", self.next_id.fetch_add(1, Ordering::SeqCst));
        {
            let mut pending = self
                .pending
                .lock()
                .map_err(|_| anyhow!("controller request lock poisoned"))?;
            if self.state.is_closed() {
                bail!("Claude controller is closed");
            }
            if pending.len() >= 1024 {
                bail!("Too many unanswered Claude controller requests");
            }
            pending.insert(id.clone(), target);
        }
        let frame = json!({ "v": 1, "id": id, "method": method, "params": params });
        if let Err(error) = self.writer.write(&frame) {
            if let Ok(mut pending) = self.pending.lock() {
                pending.remove(&id);
            }
            if !error.is::<OversizedFrame>() {
                self.emitter.stop();
            }
            return Err(error);
        }
        Ok(id)
    }

    /// Request shutdown and reap the helper within a bounded grace.
    pub(super) fn stop(&self) {
        let deadline = Instant::now() + SHUTDOWN_TIMEOUT;
        if !self.state.is_closed() {
            let _ = self.request("shutdown", json!({}));
        }
        self.state.request_shutdown(
            self.child.clone(),
            &self.pending,
            &self.workers,
            ShutdownRequest::Graceful(deadline),
        );
        self.state.wait_for_shutdown(&self.child, deadline);
    }

    /// Cancel final senders and detach pipe workers when the client goes away.
    pub(super) fn close(&self) {
        self.stop();
        self.writer.close();
        self.workers.close();
    }
}

impl Drop for ControllerTransport {
    fn drop(&mut self) {
        self.close();
    }
}

/// Record a wire frame only when the caller explicitly enables tracing.
pub(super) fn trace_frame(trace: &Trace, direction: &str, frame: &Value) {
    if let Some(trace) = trace {
        if let Ok(mut file) = trace.lock() {
            let record = format!("{}\n", json!({ "direction": direction, "message": frame }));
            let _ = file.write_all(record.as_bytes());
        }
    }
}

/// Signal a failed helper before its bounded kill-and-reap fallback.
pub(super) fn request_stop(pid: u32) {
    #[cfg(unix)]
    unsafe {
        libc::kill(pid as i32, libc::SIGTERM);
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
}

/// Reject synchronous calls without waiting for event delivery or consumer acknowledgements.
pub(super) fn reject_pending(pending: &Pending) {
    if let Ok(mut pending) = pending.lock() {
        for (_, target) in pending.drain() {
            if let RequestTarget::Reply { sender } = target {
                let _ = sender.send(Err("Claude controller closed before replying".to_string()));
            }
        }
    }
}
