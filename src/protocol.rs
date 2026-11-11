//! Small bounded JSON-RPC client for the installed native `codex app-server`.

use std::process::{Command, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use crate::backend::{BackendClient, BackendKind, RewindForkRequest};
use crate::protocol_events::EventQueue;
use crate::protocol_forms::FormWorker;
use crate::protocol_rewind::fork_before_edited_turn;
use crate::protocol_state::{request_key, ClientState, Target};
use crate::protocol_writer::InputWriter;

/// Keep the native CLI independent of Carlos's Rust dependency graph.
pub(crate) struct AppServerClient {
    state: Arc<ClientState>,
    writer: InputWriter,
    events_rx: Mutex<Option<mpsc::Receiver<String>>>,
    forms: FormWorker,
    reader: Mutex<Option<crate::protocol_socket::PendingReader>>,
}

impl AppServerClient {
    /// Start a caller-supplied process for deterministic pipe tests.
    #[cfg(test)]
    pub(crate) fn start_command(command: Command) -> Result<Self> {
        let (events, receiver) = EventQueue::new();
        let client = Self::start_with_events(command, events)?;
        *client.events_rx.lock().expect("Codex receiver poisoned") = Some(receiver);
        Ok(client)
    }

    /// Start one connection on a stable generation-stamped event stream.
    #[cfg(test)]
    pub(crate) fn start_with_events(command: Command, events: EventQueue) -> Result<Self> {
        let client = Self::prepare_with_events(command, events)?;
        client.start_reader();
        Ok(client)
    }

    /// Prepare a child outside the manager lock; publish its handle before reading callbacks.
    pub(crate) fn prepare_with_events(mut command: Command, events: EventQueue) -> Result<Self> {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Start Codex connection")?;
        let stdin = child.stdin.take().context("Missing Codex stdin")?;
        let stdout = child.stdout.take().context("Missing Codex stdout")?;
        let diagnostics = crate::codex_diagnostics::capture(child.stderr.take());
        let state = ClientState::new(child, events);
        let reader = crate::protocol_socket::PendingReader::Json(stdout, diagnostics);
        let writer = InputWriter::start(Box::new(stdin), state.clone());

        spawn_deadlines(state.clone());
        let forms = FormWorker::start(state.clone());
        Ok(Self {
            state,
            writer,
            events_rx: Mutex::new(None),
            forms,
            reader: Mutex::new(Some(reader)),
        })
    }

    /// Attach to one explicit public Unix endpoint without starting a native server.
    pub(crate) fn prepare_socket(path: &std::path::Path, events: EventQueue) -> Result<Self> {
        let parts = crate::protocol_socket::prepare(path, events)?;
        let writer = InputWriter::start(parts.input, parts.state.clone());
        spawn_deadlines(parts.state.clone());
        let forms = FormWorker::start(parts.state.clone());
        Ok(Self {
            state: parts.state,
            writer,
            forms,
            events_rx: Mutex::new(None),
            reader: Mutex::new(Some(parts.reader)),
        })
    }

    /// Start stdout only after this connection can receive callback replies.
    pub(crate) fn start_reader(&self) {
        if let Some(reader) = self.reader.lock().expect("Codex reader poisoned").take() {
            reader.start(self.state.clone());
        }
    }

    /// Include enqueueing, pipe writes and the response in one request deadline.
    pub(crate) fn call(&self, method: &str, params: Value, timeout: Duration) -> Result<String> {
        let deadline = Instant::now() + timeout;
        let id = self.state.next_id();
        let key = request_key(&id).expect("native client ID");
        let (reply, receiver) = mpsc::channel();
        let target = if history_reply(method, &params) {
            Target::History(reply)
        } else {
            Target::Caller(reply)
        };
        self.state.insert(key.clone(), target)?;
        if let Err(error) = self.write(json!({"id":id,"method":method,"params":params}), deadline) {
            self.state.take(&key);
            self.state.finish_large_reply(&key);
            return Err(error);
        }
        match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(error)) => bail!(error),
            Err(_) => {
                self.state.take(&key);
                let error = timeout_message(method);
                if timeout_closes_connection(method) {
                    self.state.close(
                        &format!("{error}; request may already have been accepted"),
                        true,
                    );
                }
                bail!(error);
            }
        }
    }

    /// Return a native callback answer with its original string or numeric ID.
    pub(crate) fn respond(&self, request_id: &Value, result: Value) -> Result<()> {
        self.write(
            json!({"id":request_id,"result":result}),
            Instant::now() + Duration::from_secs(2),
        )
    }

    /// Return a native callback error without coercing its ID.
    pub(crate) fn respond_error(&self, request_id: &Value, code: i64, message: &str) -> Result<()> {
        let result = self.write(
            json!({"id":request_id,"error":{"code":code,"message":message}}),
            Instant::now() + Duration::from_secs(2),
        );
        if let Err(error) = &result {
            self.state.close(
                &format!("Could not return native callback error: {error}"),
                true,
            );
        }
        result
    }

    /// Take the raw native event stream exactly once.
    pub(crate) fn take_events_rx(&mut self) -> Result<mpsc::Receiver<String>> {
        self.events_rx
            .lock()
            .expect("Codex receiver poisoned")
            .take()
            .context("Codex events receiver already taken")
    }

    /// Stop without acquiring a possibly blocked stdin pipe.
    pub(crate) fn stop(&self) {
        self.state.close("Codex app-server stopped", false);
        let deadline = Instant::now() + Duration::from_millis(250);
        while !self.state.reap() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
    }

    fn write(&self, frame: Value, deadline: Instant) -> Result<()> {
        self.writer.write(frame, deadline, &self.state)
    }

    fn request_async(
        &self,
        method: &str,
        params: Value,
        context: Value,
        raw: bool,
    ) -> Result<String> {
        let id = self.state.next_id();
        let key = request_key(&id).expect("native client ID");
        let timeout = context["timeoutMs"]
            .as_u64()
            .unwrap_or(15000)
            .clamp(1, 15000);
        let deadline = Instant::now() + Duration::from_millis(timeout);
        let (identity, retained_bytes) = control_identity(method, &params, &context)?;
        self.state.insert(
            key.clone(),
            Target::Event {
                method: method.into(),
                params: identity,
                context,
                bytes: retained_bytes,
                deadline,
                raw,
            },
        )?;
        let write = self.writer.enqueue(
            json!({"id":id,"method":method,"params":params}),
            deadline,
            &self.state,
        );
        let receiver = match write {
            Ok(receiver) => receiver,
            Err(error) => {
                self.state.take(&key);
                return Err(error);
            }
        };
        let state = self.state.clone();
        thread::spawn(move || {
            let _ = crate::protocol_writer::wait(receiver, deadline, &state);
        });
        Ok(id.as_str().expect("client ID string").into())
    }
}

fn control_identity(method: &str, params: &Value, context: &Value) -> Result<(Value, usize)> {
    let bytes = params.to_string().len();
    let maximum = if matches!(method, "thread/queue/add" | "thread/queue/update") {
        crate::protocol_writer::MAX_FRAME_BYTES.saturating_sub(1024)
    } else {
        65536
    };
    anyhow::ensure!(
        bytes <= maximum,
        "Codex control params exceed {maximum} bytes"
    );
    let context_bytes = context.to_string().len();
    anyhow::ensure!(
        context_bytes <= 65536,
        "Codex local control context exceeds 64 KiB"
    );
    let mut identity = params.clone();
    if matches!(method, "thread/queue/add" | "thread/queue/update") {
        identity
            .as_object_mut()
            .context("Queue params must be an object")?
            .remove("input");
    }
    let retained_bytes = identity.to_string().len() + context_bytes + method.len();
    Ok((identity, retained_bytes))
}

impl Drop for AppServerClient {
    fn drop(&mut self) {
        self.stop();
    }
}

impl BackendClient for AppServerClient {
    fn kind(&self) -> BackendKind {
        BackendKind::Codex
    }
    fn call(&self, method: &str, params: Value, timeout: Duration) -> Result<String> {
        AppServerClient::call(self, method, params, timeout)
    }
    fn notify(&self, method: &str, params: Value) -> Result<()> {
        self.write(
            json!({"method":method,"params":params}),
            Instant::now() + Duration::from_millis(500),
        )
    }
    fn validate_form(
        &self,
        id: Value,
        schema: Value,
        answer: Option<Value>,
        revision: u64,
    ) -> Result<()> {
        self.forms.check(id, schema, answer, revision)
    }
    fn request_rate_limits(&self) -> Result<Option<String>> {
        self.request_async("account/rateLimits/read", json!({}), Value::Null, true)
            .map(Some)
    }
    fn request_native_control(&self, method: &str, params: Value) -> Result<()> {
        self.request_async(method, params, Value::Null, false)
            .map(|_| ())
    }
    fn request_control_with_context(
        &self,
        method: &str,
        params: Value,
        context: Value,
    ) -> Result<()> {
        self.request_async(method, params, context, false)
            .map(|_| ())
    }
    fn acknowledge_event(&self, length: usize) {
        self.state.events.handled(length);
    }
    fn fork_from_rewind(
        &self,
        thread_id: &str,
        request: RewindForkRequest,
        timeout: Duration,
    ) -> Result<String> {
        fork_before_edited_turn(self, thread_id, request, timeout)
    }
    fn respond(&self, request_id: &Value, result: Value) -> Result<()> {
        AppServerClient::respond(self, request_id, result)
    }
    fn respond_error(&self, request_id: &Value, code: i64, message: &str) -> Result<()> {
        AppServerClient::respond_error(self, request_id, code, message)
    }
    fn take_events_rx(&mut self) -> Result<mpsc::Receiver<String>> {
        AppServerClient::take_events_rx(self)
    }
    fn stop(&mut self) {
        AppServerClient::stop(self);
    }
}

/// Only these known reads can time out without leaving an uncertain mutation.
pub(crate) fn is_read_only(method: &str) -> bool {
    matches!(
        method,
        "model/list"
            | "account/rateLimits/read"
            | "thread/read"
            | "thread/list"
            | "thread/items/list"
            | "thread/items/read"
            | "thread/turns/list"
            | "thread/queue/list"
            | "thread/backgroundTerminals/list"
            | "thread/goal/get"
    )
}

/// Known targeted stops may expire without killing unrelated work; pipe failures still close.
pub(crate) fn timeout_closes_connection(method: &str) -> bool {
    !is_read_only(method)
        && !matches!(
            method,
            "turn/interrupt" | "thread/backgroundTerminals/terminate"
        )
}

/// Report uncertain stop completion without silently retrying another target.
pub(crate) fn timeout_message(method: &str) -> String {
    if matches!(
        method,
        "turn/interrupt" | "thread/backgroundTerminals/terminate"
    ) {
        "Codex stop not confirmed; it may still complete. Refresh native state before retrying"
            .into()
    } else {
        format!("Timeout waiting for Codex {method}")
    }
}

fn spawn_deadlines(state: Arc<ClientState>) {
    thread::spawn(move || {
        while !state.is_closed() {
            state.expire();
            thread::sleep(Duration::from_millis(25));
        }
        while !state.reap() {
            thread::sleep(Duration::from_millis(25));
        }
    });
}

#[cfg(test)]
#[path = "tests/protocol_transport_tests.rs"]
mod tests;

// Only blocking history reads bypass the ordinary event-frame budget.
fn history_reply(method: &str, params: &Value) -> bool {
    method == "thread/items/list"
        || (method == "thread/turns/list" && params["itemsView"] == "full")
        || (method == "thread/read" && params["includeTurns"] == true)
}

#[cfg(test)]
#[path = "tests/protocol_history_transport_tests.rs"]
mod history_transport_tests;
