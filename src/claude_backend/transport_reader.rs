//! Routes accepted controller frames through one ordered emitter before queueing them.

use std::io::{BufRead, BufReader, Read};
use std::process::{ChildStderr, ChildStdout};
use std::sync::{Arc, Mutex};

use anyhow::Result;
use serde_json::Value;

use super::conversation::ConversationGuard;
use super::event_emitter::EventEmitter;
use super::transport::{trace_frame, ControllerTransport, Pending, RequestTarget, Trace};
use super::transport_state::TransportState;

struct Reader {
    emitter: Arc<EventEmitter>,
    pending: Pending,
    state: Arc<TransportState>,
    trace: Trace,
    conversation: Arc<Mutex<ConversationGuard>>,
}

/// Read helper stdout and stderr independently, sharing one ordered batch emitter.
pub(super) fn spawn_readers(
    stdout: ChildStdout,
    stderr: ChildStderr,
    transport: &ControllerTransport,
) {
    let reader = Reader {
        emitter: transport.emitter.clone(),
        pending: transport.pending.clone(),
        state: transport.state.clone(),
        trace: transport.trace.clone(),
        conversation: transport.conversation.clone(),
    };
    let emitter = transport.emitter.clone();
    transport.workers.spawn(move || reader.run(stdout));
    transport.workers.spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut chunk = [0_u8; 4096];
        while let Ok(count) = reader.read(&mut chunk) {
            if count == 0 {
                break;
            }
            let text = String::from_utf8_lossy(&chunk[..count]);
            emitter.failure(&format!("Claude controller: {}", text.trim()));
        }
    });
}

impl Reader {
    fn run(self, stdout: ChildStdout) {
        let mut closed = false;
        for line in BufReader::new(stdout).lines() {
            let frame = line
                .ok()
                .and_then(|line| serde_json::from_str::<Value>(&line).ok());
            let Some(frame) = frame.filter(|frame| frame["v"] == 1) else {
                self.stop_and_report("Claude controller sent an invalid protocol frame");
                break;
            };
            closed |= frame["event"] == "closed";
            trace_frame(&self.trace, "server", &frame);
            if let Err(error) = self.route(frame) {
                if !self.state.shutdown_requested() {
                    self.stop_and_report(&error.to_string());
                }
                break;
            }
        }
        self.state.stdout_closed(&self.pending);
        self.emitter.reader_eof_checkpoint();
        if !closed {
            self.emitter
                .exited("Claude controller stdout closed unexpectedly");
        }
        self.emitter.reader_done_checkpoint();
    }

    fn route(&self, frame: Value) -> Result<()> {
        if frame["type"] == "reply" {
            return self.route_reply(frame);
        }
        let frames = self
            .conversation
            .lock()
            .map_err(|_| anyhow::anyhow!("session guard lock poisoned"))?
            .route(frame)?;
        for (frame, epoch) in frames {
            self.emitter.route(&frame, None, &Value::Null, epoch)?;
        }
        Ok(())
    }

    fn route_reply(&self, frame: Value) -> Result<()> {
        let target = frame["id"]
            .as_str()
            .and_then(|id| self.pending.lock().ok()?.remove(id));
        match target {
            Some(RequestTarget::Reply { sender }) => {
                let result = if frame["ok"] == true {
                    Ok(frame["result"].clone())
                } else {
                    Err(frame["error"]["message"]
                        .as_str()
                        .unwrap_or("Claude SDK request failed")
                        .to_string())
                };
                let _ = sender.send(result);
            }
            Some(RequestTarget::Event {
                method,
                context,
                epoch,
            }) => self.emitter.route(&frame, Some(&method), &context, epoch)?,
            None => self.emitter.route(&frame, None, &Value::Null, 0)?,
        }
        Ok(())
    }

    fn stop_and_report(&self, message: &str) {
        self.emitter.stop_and_report(message);
    }
}
