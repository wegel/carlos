//! Own one native app-server or one local socket connection across reconnects.

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use anyhow::{ensure, Context, Result};
use serde_json::json;

use crate::codex_snapshot::CodexSnapshot;
use crate::protocol::AppServerClient;
use crate::protocol_events::EventQueue;

#[path = "codex_connection_backend.rs"]
mod backend;

#[derive(Clone, Debug)]
/// An owned subprocess stops with Carlos; an attached native server belongs to someone else.
pub(crate) enum ConnectionMode {
    Owned(crate::protocol_capabilities::OwnedFeatures),
    Socket(PathBuf),
}

/// Keep one UI stream and one snapshot slot for the whole Carlos process.
pub(crate) struct CodexClient {
    shared: Arc<Shared>,
    receiver: Option<mpsc::Receiver<String>>,
}

struct Shared {
    mode: ConnectionMode,
    current: Mutex<Option<Arc<AppServerClient>>>,
    events: EventQueue,
    generation: AtomicU64,
    reconnecting: AtomicBool,
    stopped: AtomicBool,
    snapshot: Mutex<Option<(u64, CodexSnapshot)>>,
}

impl CodexClient {
    /// Start the default CLI or attach only to an explicit local socket.
    pub(crate) fn start(socket: Option<PathBuf>) -> Result<Self> {
        let mode = match socket {
            Some(path) => ConnectionMode::Socket(path),
            None => ConnectionMode::Owned(crate::protocol_capabilities::owned_features()),
        };
        let (events, receiver) = EventQueue::new();
        let client = Arc::new(mode.prepare(events.for_generation(1))?);
        client.start_reader();
        Ok(Self {
            shared: Arc::new(Shared {
                mode,
                current: Mutex::new(Some(client)),
                events,
                generation: AtomicU64::new(1),
                reconnecting: AtomicBool::new(false),
                stopped: AtomicBool::new(false),
                snapshot: Mutex::new(None),
            }),
            receiver: Some(receiver),
        })
    }

    /// Finish initial resume history off the UI thread without repeating initialize or resume.
    pub(crate) fn bootstrap(&self, thread: String, runtime: String) -> Result<()> {
        ensure!(
            !self.shared.reconnecting.swap(true, Ordering::SeqCst),
            "Codex rebuild already pending"
        );
        let client = self.connection()?;
        let shared = self.shared.clone();
        let generation = shared.generation.load(Ordering::SeqCst);
        thread::spawn(move || {
            let result = crate::codex_snapshot::read_initialized(&client, &thread, runtime)
                .and_then(|snapshot| publish(&shared, generation, &thread, snapshot));
            finish_attempt(&shared, generation, result);
        });
        Ok(())
    }

    fn connection(&self) -> Result<Arc<AppServerClient>> {
        self.shared
            .current
            .lock()
            .expect("Codex connection poisoned")
            .clone()
            .context("Codex connection is unavailable; F5 reconnects")
    }

    fn reconnect(&self, thread: String) -> Result<u64> {
        ensure!(
            !self.shared.stopped.load(Ordering::SeqCst),
            "Carlos has stopped"
        );
        ensure!(
            !self.shared.reconnecting.swap(true, Ordering::SeqCst),
            "Codex reconnect already pending"
        );
        let generation = self.shared.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let shared = self.shared.clone();
        thread::spawn(move || {
            let result = rebuild(&shared, generation, &thread);
            finish_attempt(&shared, generation, result);
        });
        Ok(generation)
    }

    fn stop(&self) {
        self.shared.stopped.store(true, Ordering::SeqCst);
        self.shared.generation.fetch_add(1, Ordering::SeqCst);
        let client = self
            .shared
            .current
            .lock()
            .expect("Codex connection poisoned")
            .take();
        if let Some(client) = client {
            client.stop();
        }
        self.shared
            .snapshot
            .lock()
            .expect("Codex snapshot poisoned")
            .take();
    }
}

impl Drop for CodexClient {
    fn drop(&mut self) {
        self.stop();
    }
}

fn rebuild(shared: &Shared, generation: u64, thread: &str) -> Result<()> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let old = shared
        .current
        .lock()
        .expect("Codex connection poisoned")
        .take();
    if let Some(old) = old {
        old.stop();
    }
    shared
        .snapshot
        .lock()
        .expect("Codex snapshot poisoned")
        .take();
    let client = Arc::new(
        shared
            .mode
            .prepare(shared.events.for_generation(generation))?,
    );
    {
        let mut current = shared.current.lock().expect("Codex connection poisoned");
        if shared.stopped.load(Ordering::SeqCst)
            || shared.generation.load(Ordering::SeqCst) != generation
        {
            drop(current);
            client.stop();
            anyhow::bail!("Carlos stopped during reconnect");
        }
        *current = Some(client.clone());
    }
    client.start_reader();
    let snapshot = crate::codex_snapshot::read(&client, thread, deadline)?;
    publish(shared, generation, thread, snapshot)
}

fn publish(shared: &Shared, generation: u64, thread: &str, snapshot: CodexSnapshot) -> Result<()> {
    ensure!(
        !shared.stopped.load(Ordering::SeqCst)
            && shared.generation.load(Ordering::SeqCst) == generation,
        "Codex reconnect was retired"
    );
    *shared.snapshot.lock().expect("Codex snapshot poisoned") = Some((generation, snapshot));
    shared.events.for_generation(generation).send(
        json!({
            "method":"carlos/codex/ready","params":{"generation":generation,"threadId":thread}
        })
        .to_string(),
    )
}

fn finish_attempt(shared: &Shared, generation: u64, result: Result<()>) {
    if let Err(error) = result {
        if shared.generation.load(Ordering::SeqCst) == generation {
            let client = shared
                .current
                .lock()
                .expect("Codex connection poisoned")
                .take();
            if let Some(client) = client {
                client.stop();
            }
            shared
                .snapshot
                .lock()
                .expect("Codex snapshot poisoned")
                .take();
            shared.events.for_generation(generation).terminal(json!({
                "method":"carlos/codex/closed","params":{"message":crate::protocol_events::bounded_diagnostic(&format!("Reconnect failed: {error:#}"))}
            }).to_string());
        }
    }
    shared.reconnecting.store(false, Ordering::SeqCst);
}

impl ConnectionMode {
    fn prepare(&self, events: EventQueue) -> Result<AppServerClient> {
        match self {
            Self::Owned(features) => {
                AppServerClient::prepare_with_events(owned_command(features), events)
            }
            Self::Socket(path) => AppServerClient::prepare_socket(path, events),
        }
    }
}

/// Reuse the flags probed before the UI starts; reconnect never leaves a probe child behind.
fn owned_command(features: &crate::protocol_capabilities::OwnedFeatures) -> Command {
    let mut command = Command::new("codex");
    if features.live_settings {
        command.args(["--enable", "step_model_switching"]);
    }
    if features.enable_goals {
        command.args(["--enable", "goals"]);
    }
    command.arg("app-server");
    command
}

#[cfg(test)]
#[path = "tests/codex_connection_tests.rs"]
mod tests;
