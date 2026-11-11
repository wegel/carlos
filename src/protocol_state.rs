//! Share native process lifetime and waiting Codex requests across pipe workers.

use std::collections::{HashMap, HashSet};
use std::process::Child;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Instant;

use anyhow::{ensure, Result};
use serde_json::{json, Value};

use crate::protocol_events::EventQueue;

/// Carry a complete native reply or a terminal pipe error to one waiting caller.
pub(crate) type Reply = std::result::Result<String, String>;

/// Retain the exact JSON type of an RPC ID rather than coercing strings/numbers.
pub(crate) fn request_key(id: &Value) -> Option<String> {
    (id.is_string() || id.is_number()).then(|| id.to_string())
}

/// Route a reply to a blocking caller or back through the UI event channel.
pub(crate) enum Target {
    Caller(mpsc::Sender<Reply>),
    History(mpsc::Sender<Reply>),
    Event {
        method: String,
        params: Value,
        context: Value,
        bytes: usize,
        deadline: Instant,
        raw: bool,
    },
}

#[derive(Default)]
struct Requests {
    closed: Option<String>,
    pending: HashMap<String, Target>,
    large_replies: HashSet<String>,
}

enum Endpoint {
    Process(Child),
    #[cfg(unix)]
    Socket(std::os::unix::net::UnixStream),
}

/// Own the process and settle all pending calls once when either pipe fails.
pub(crate) struct ClientState {
    child: Mutex<Endpoint>,
    requests: Mutex<Requests>,
    pub(crate) events: EventQueue,
    issued: AtomicU64,
}

impl ClientState {
    /// Share the child handle without letting a blocked stdin write hold it.
    pub(crate) fn new(child: Child, events: EventQueue) -> Arc<Self> {
        Arc::new(Self {
            child: Mutex::new(Endpoint::Process(child)),
            requests: Mutex::new(Requests::default()),
            events,
            issued: AtomicU64::new(0),
        })
    }

    /// Retain a shutdown handle for a local socket without owning its native server.
    #[cfg(unix)]
    pub(crate) fn new_socket(
        socket: std::os::unix::net::UnixStream,
        events: EventQueue,
    ) -> Arc<Self> {
        Arc::new(Self {
            child: Mutex::new(Endpoint::Socket(socket)),
            requests: Mutex::new(Requests::default()),
            events,
            issued: AtomicU64::new(0),
        })
    }

    /// Issue a private client ID; native callback IDs use a separate direction.
    pub(crate) fn next_id(&self) -> Value {
        json!(format!(
            "carlos-rpc-{}",
            self.issued.fetch_add(1, Ordering::SeqCst) + 1
        ))
    }

    /// Discard late replies to retired calls without keeping an unbounded ID set.
    pub(crate) fn was_issued(&self, id: &Value) -> bool {
        id.as_str()
            .and_then(|id| id.strip_prefix("carlos-rpc-"))
            .and_then(|number| number.parse::<u64>().ok())
            .is_some_and(|number| number > 0 && number <= self.issued.load(Ordering::SeqCst))
    }

    /// Reject new calls after terminal closure or when 64 requests already wait.
    pub(crate) fn insert(&self, key: String, target: Target) -> Result<()> {
        let mut requests = self.requests.lock().expect("Codex requests poisoned");
        ensure!(
            requests.closed.is_none(),
            "{}",
            requests.closed.as_deref().unwrap_or("Codex closed")
        );
        ensure!(requests.pending.len() < 64, "Codex has 64 pending requests");
        let bytes = |target: &Target| match target {
            Target::Event { bytes, .. } => *bytes,
            Target::Caller(_) | Target::History(_) => 0,
        };
        ensure!(
            requests.pending.values().map(bytes).sum::<usize>() + bytes(&target)
                <= 16 * 1024 * 1024,
            "Codex pending control context exceeds 16 MiB"
        );
        if matches!(target, Target::History(_)) {
            ensure!(
                !requests
                    .pending
                    .values()
                    .any(|target| matches!(target, Target::History(_))),
                "Codex already has an active large history read"
            );
            ensure!(
                requests.large_replies.len() < 64,
                "Codex has 64 undrained history reads"
            );
            requests.large_replies.insert(key.clone());
        }
        requests.pending.insert(key, target);
        Ok(())
    }

    /// Remove a caller when its reply arrives or its deadline expires.
    pub(crate) fn take(&self, key: &str) -> Option<Target> {
        self.requests.lock().ok()?.pending.remove(key)
    }

    /// Consume only the allowance for this exact public history response ID.
    pub(crate) fn finish_large_reply(&self, key: &str) -> bool {
        self.requests
            .lock()
            .expect("Codex requests poisoned")
            .large_replies
            .remove(key)
    }

    /// Report whether either pipe has closed.
    pub(crate) fn is_closed(&self) -> bool {
        self.requests
            .lock()
            .map_or(true, |state| state.closed.is_some())
    }

    /// Close once, kill the owned process, settle callers and publish one notice.
    pub(crate) fn close(&self, message: &str, notify: bool) {
        let pending = {
            let mut requests = self.requests.lock().expect("Codex requests poisoned");
            if requests.closed.is_some() {
                return;
            }
            requests.closed = Some(message.to_string());
            std::mem::take(&mut requests.pending)
        };
        self.kill();
        for (key, target) in pending {
            self.reply(&key, target, Err(message.to_string()));
        }
        if notify {
            self.events.terminal(
                json!({"method":"carlos/codex/closed","params":{"message":message}}).to_string(),
            );
        }
    }

    /// Deliver a native reply without dropping unknown fields.
    pub(crate) fn reply(&self, key: &str, target: Target, reply: Reply) {
        match target {
            Target::Caller(sender) | Target::History(sender) => {
                let _ = sender.send(reply);
            }
            Target::Event {
                method,
                params,
                context,
                raw,
                ..
            } => {
                let frame = match reply {
                    Ok(line) => serde_json::from_str::<Value>(&line).expect("parsed native reply"),
                    Err(message) => {
                        json!({"id":serde_json::from_str::<Value>(key).expect("RPC ID"),
                        "error":{"code":-32000,"message":message}})
                    }
                };
                let event = if raw {
                    frame.to_string()
                } else {
                    json!({"method":"carlos/codex/controlReply",
                    "params":{"requestMethod":method,"requestParams":params,"requestContext":context,"response":frame}})
                    .to_string()
                };
                if let Err(error) = self.events.send(event) {
                    self.close(&error.to_string(), true);
                }
            }
        }
    }

    /// Expire optional async reads without blocking the reader or closing a healthy CLI.
    pub(crate) fn expire(&self) {
        let expired = {
            let mut requests = self.requests.lock().expect("Codex requests poisoned");
            let keys: Vec<String> = requests
                .pending
                .iter()
                .filter(|(_, target)| {
                    matches!(target, Target::Event { deadline, .. } if *deadline <= Instant::now())
                })
                .map(|(key, _)| key.clone())
                .collect();
            keys.into_iter()
                .filter_map(|key| requests.pending.remove(&key).map(|target| (key, target)))
                .collect::<Vec<_>>()
        };
        for (key, target) in expired {
            if let Target::Event { method, .. } = &target {
                if crate::protocol::timeout_closes_connection(method) {
                    self.reply(
                        &key,
                        target,
                        Err(
                            "Codex control timed out; request may already have been accepted"
                                .into(),
                        ),
                    );
                    self.close(
                        "Codex control timed out; request may already have been accepted",
                        true,
                    );
                    continue;
                }
            }
            let message = match &target {
                Target::Event { method, .. } => crate::protocol::timeout_message(method),
                _ => "Codex control reply timed out".into(),
            };
            self.reply(&key, target, Err(message));
        }
    }

    fn kill(&self) {
        if let Ok(mut endpoint) = self.child.lock() {
            match &mut *endpoint {
                Endpoint::Process(child) => {
                    if !matches!(child.try_wait(), Ok(Some(_))) {
                        let _ = child.kill();
                    }
                }
                #[cfg(unix)]
                Endpoint::Socket(socket) => {
                    let _ = socket.shutdown(std::net::Shutdown::Both);
                }
            }
        }
    }

    /// Reap only the owned process; socket shutdown never waits for the shared server.
    pub(crate) fn reap(&self) -> bool {
        self.child
            .lock()
            .ok()
            .is_none_or(|mut endpoint| match &mut *endpoint {
                Endpoint::Process(child) => matches!(child.try_wait(), Ok(Some(_))),
                #[cfg(unix)]
                Endpoint::Socket(_) => true,
            })
    }
}
