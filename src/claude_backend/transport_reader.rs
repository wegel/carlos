//! Routes controller replies and preserves SDK frames for the Rust UI.

use std::io::{BufRead, BufReader, Read};
use std::process::{Child, ChildStderr, ChildStdout};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::Result;
use serde_json::{json, Value};

use super::conversation::ConversationGuard;
use super::event_queue::EventSender;
use super::transport::{
    abort_helper, trace_frame, ControllerTransport, Pending, RequestTarget, Trace,
};

struct Reader {
    events: EventSender,
    pending: Pending,
    stopped: Arc<AtomicBool>,
    trace: Trace,
    child: Arc<Mutex<Child>>,
    conversation: Arc<Mutex<ConversationGuard>>,
}

/// Read helper stdout and stderr independently of the UI thread.
pub(super) fn spawn_readers(
    stdout: ChildStdout,
    stderr: ChildStderr,
    events: EventSender,
    transport: &ControllerTransport,
) {
    let reader = Reader {
        events: events.clone(),
        pending: transport.pending.clone(),
        stopped: transport.stopped.clone(),
        trace: transport.trace.clone(),
        child: transport.child.clone(),
        conversation: transport.conversation.clone(),
    };
    let child = transport.child.clone();
    thread::spawn(move || reader.run(stdout));
    thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut chunk = [0_u8; 4096];
        while let Ok(count) = reader.read(&mut chunk) {
            if count == 0 {
                break;
            }
            let text = String::from_utf8_lossy(&chunk[..count]);
            if events
                .send(error_frame(&format!("Claude controller: {}", text.trim())))
                .is_err()
            {
                stop_and_report(
                    &events,
                    child.clone(),
                    "Claude UI event backlog overflowed; session stopped",
                );
                break;
            }
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
                stop_and_report(
                    &self.events,
                    self.child.clone(),
                    "Claude controller sent an invalid protocol frame",
                );
                break;
            };
            trace_frame(&self.trace, "server", &frame);
            closed |= frame["event"] == "closed";
            if let Err(error) = self.route(frame) {
                stop_and_report(&self.events, self.child.clone(), &error.to_string());
                break;
            }
        }
        self.stopped.store(true, Ordering::SeqCst);
        if let Ok(mut pending) = self.pending.lock() {
            for (_, target) in pending.drain() {
                if let RequestTarget::Reply(sender) = target {
                    let _ =
                        sender.send(Err("Claude controller closed before replying".to_string()));
                }
            }
        }
        if !closed {
            let _ = self
                .events
                .send(error_frame("Claude controller stdout closed unexpectedly"));
        }
    }

    fn route(&self, frame: Value) -> Result<()> {
        if frame["type"] == "reply" {
            return route_reply(&self.events, &self.pending, frame);
        }
        let frames = self
            .conversation
            .lock()
            .map_err(|_| anyhow::anyhow!("session guard lock poisoned"))?
            .route(frame)?;
        for (frame, epoch) in frames {
            let mut event: Value = serde_json::from_str(&envelope(&frame, None))?;
            event["params"]["epoch"] = json!(epoch);
            self.events.send(event.to_string()).map_err(|_| {
                anyhow::anyhow!("Claude UI event backlog overflowed; session stopped")
            })?;
        }
        Ok(())
    }
}

fn route_reply(events: &EventSender, pending: &Pending, frame: Value) -> Result<()> {
    let target = frame["id"]
        .as_str()
        .and_then(|id| pending.lock().ok()?.remove(id));
    match target {
        Some(RequestTarget::Reply(sender)) => {
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
        }) => {
            let mut event: Value = serde_json::from_str(&envelope(&frame, Some(&method)))?;
            event["params"]["epoch"] = json!(epoch);
            event["params"]["requestContext"] = context;
            events.send(event.to_string())?;
        }
        None => events.send(envelope(&frame, None))?,
    }
    Ok(())
}

/// Keep a complete helper frame inside a Claude-specific UI message.
pub(super) fn envelope(frame: &Value, method: Option<&str>) -> String {
    json!({ "method": "claude/controller", "params": { "frame": frame, "requestMethod": method } })
        .to_string()
}

fn error_frame(message: &str) -> String {
    envelope(
        &json!({ "v": 1, "type": "event", "event": "transport_error", "error": { "message": message } }),
        None,
    )
}

fn stop_and_report(events: &EventSender, child: Arc<Mutex<Child>>, message: &str) {
    abort_helper(child);
    let sender = events.sender.clone();
    let error = events.stamp(error_frame(message));
    // One final frame may wait for a full queue; shutdown never waits for this thread.
    thread::spawn(move || {
        let _ = sender.send(error);
    });
}
