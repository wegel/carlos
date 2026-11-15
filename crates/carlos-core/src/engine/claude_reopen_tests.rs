//! Reopen admission observes the live helper even before manager facts catch up.

use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use carlos_proto::submissions::{ClientOperation, OperationResult, OperationState};
use serde_json::Value;

use super::{EngineCommand, EngineHandle, EngineReply};
use crate::backend::{BackendClient, BackendKind, EventReceiver};
use crate::backend_commands::{BackendCommands, CommandHandle};
use crate::dispatch_guard::DispatchGuard;
use crate::session::state::SessionState;

const TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone)]
struct Peer {
    generation: u64,
    started: mpsc::Sender<()>,
    release: Arc<(Mutex<bool>, Condvar)>,
}

struct Client(Peer);

#[test]
fn new_helper_with_old_closed_facts_attaches_without_another_reopen() {
    let (engine, peer, started, _events) = fixture(2);
    open(&engine, 1);
    assert!(matches!(
        outcome(&engine, 1).state,
        OperationState::Accepted { .. }
    ));
    assert!(
        started.try_recv().is_err(),
        "no worker starts in the slot-install/fact-forwarding gap"
    );
    stop(engine, peer);
}

#[test]
fn concurrent_open_admitted_during_failed_worker_attaches_and_only_a_later_open_retries() {
    let (engine, peer, started, _events) = fixture(1);
    open(&engine, 1);
    started.recv_timeout(TIMEOUT).expect("first reopen starts");
    open(&engine, 2);
    assert!(matches!(
        outcome(&engine, 2).state,
        OperationState::Accepted { .. }
    ));
    assert!(
        started.try_recv().is_err(),
        "second open attaches while the first worker waits"
    );
    release(&peer);
    assert!(matches!(
        outcome(&engine, 1).state,
        OperationState::Rejected { .. }
    ));
    open(&engine, 3);
    started
        .recv_timeout(TIMEOUT)
        .expect("a later explicit open may retry");
    assert!(matches!(
        outcome(&engine, 3).state,
        OperationState::Rejected { .. }
    ));
    stop(engine, peer);
}

fn fixture(generation: u64) -> (EngineHandle, Peer, mpsc::Receiver<()>, mpsc::Sender<String>) {
    let (started, requests) = mpsc::channel();
    let peer = Peer {
        generation,
        started,
        release: Arc::default(),
    };
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.enable_claude();
    session.close_claude();
    session.events.backend_closed = true;
    let (events, receiver) = mpsc::channel();
    let (engine, _) = EngineHandle::spawn(session, Box::new(Client(peer.clone())), receiver.into());
    (engine, peer, requests, events)
}

fn open(engine: &EngineHandle, id: u64) {
    let reply = engine
        .request(EngineCommand::Reopen {
            operation: operation(id),
            generation: None,
        })
        .expect("ordinary admission")
        .recv_timeout(TIMEOUT)
        .expect("responsive owner");
    assert!(matches!(reply, EngineReply::Operation(_)));
}

fn operation(id: u64) -> ClientOperation {
    ClientOperation {
        client: "11111111111111111111111111111111".parse().expect("client"),
        id: id.to_string().parse().expect("counter"),
    }
}

fn outcome(engine: &EngineHandle, id: u64) -> OperationResult {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let EngineReply::Outcomes(results) = engine
            .request(EngineCommand::Outcomes)
            .expect("read admission")
            .recv_timeout(TIMEOUT)
            .expect("responsive reads")
        else {
            panic!("private outcomes")
        };
        if let Some(result) = results.into_iter().find(|result| {
            result.operation == operation(id)
                && !matches!(
                    result.state,
                    OperationState::Pending | OperationState::Queued
                )
        }) {
            return result;
        }
        assert!(Instant::now() < deadline, "original operation settles");
        std::thread::yield_now();
    }
}

fn release(peer: &Peer) {
    *peer.release.0.lock().expect("gate") = true;
    peer.release.1.notify_all();
}

fn stop(engine: EngineHandle, peer: Peer) {
    release(&peer);
    engine.shutdown();
    engine.join().expect("owner stops");
}

impl BackendCommands for Peer {
    fn kind(&self) -> BackendKind {
        BackendKind::Claude
    }
    fn guarded(&self, _: DispatchGuard) -> Result<CommandHandle> {
        Ok(CommandHandle::new(self.clone()))
    }
    fn reopen_session(&self, _: &str, _: Option<String>, _: Option<String>) -> Result<u64> {
        self.started.send(()).expect("worker observed");
        let guard = self.release.0.lock().expect("gate");
        let (guard, timeout) = self
            .release
            .1
            .wait_timeout_while(guard, TIMEOUT, |released| !*released)
            .expect("bounded wait");
        assert!(
            *guard && !timeout.timed_out(),
            "test releases original worker"
        );
        bail!("fixture replacement failed before install")
    }
    fn call(&self, _: &str, _: Value, _: Duration) -> Result<String> {
        bail!("no native call expected")
    }
    fn respond(&self, _: &Value, _: Value) -> Result<()> {
        bail!("no callback expected")
    }
    fn respond_error(&self, _: &Value, _: i64, _: &str) -> Result<()> {
        bail!("no callback expected")
    }
}

impl BackendClient for Client {
    fn kind(&self) -> BackendKind {
        BackendKind::Claude
    }
    fn connection_generation(&self) -> Option<u64> {
        Some(self.0.generation)
    }
    fn capture_commands(&self) -> Result<CommandHandle> {
        Ok(CommandHandle::new(self.0.clone()))
    }
    fn call(&self, _: &str, _: Value, _: Duration) -> Result<String> {
        bail!("no native call expected")
    }
    fn respond(&self, _: &Value, _: Value) -> Result<()> {
        bail!("no callback expected")
    }
    fn respond_error(&self, _: &Value, _: i64, _: &str) -> Result<()> {
        bail!("no callback expected")
    }
    fn take_events_rx(&mut self) -> Result<EventReceiver> {
        bail!("receiver supplied separately")
    }
    fn stop(&mut self) {}
}
