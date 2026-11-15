//! Keep optional quota writers bounded and independent of the session owner's read loop.

use std::sync::atomic::Ordering;
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use serde_json::Value;

use super::owner::{Channels, Owner};
use super::{EngineCommand, EngineHandle, EngineReply};
use crate::backend::{BackendClient, BackendKind, EventReceiver};
use crate::backend_commands::{BackendCommands, CommandHandle};
use crate::session::actions::ServerRequestAction;
use crate::session::claude::QuotaReadKey;
use crate::session::state::SessionState;
use crate::session::test_backend::TestBackend;

const TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Default)]
struct Gate {
    released: Mutex<bool>,
    changed: Condvar,
}

impl Gate {
    fn release(&self) {
        *self.released.lock().expect("writer gate") = true;
        self.changed.notify_all();
    }
}

struct Commands {
    gate: Arc<Gate>,
    started: mpsc::Sender<Value>,
}

impl BackendCommands for Commands {
    fn kind(&self) -> BackendKind {
        BackendKind::Claude
    }
    fn call(&self, _: &str, _: Value, _: Duration) -> Result<String> {
        bail!("no ordinary native request expected")
    }
    fn request_native_control(&self, method: &str, params: Value) -> Result<()> {
        if method == "usage.optional" {
            self.started.send(params).expect("record quota writer");
            let guard = self.gate.released.lock().expect("writer gate");
            let (guard, timeout) = self
                .gate
                .changed
                .wait_timeout_while(guard, TIMEOUT, |ready| !*ready)
                .expect("bounded native writer");
            assert!(
                *guard && !timeout.timed_out(),
                "test releases writer before its deadline"
            );
        }
        Ok(())
    }
    fn respond(&self, _: &Value, _: Value) -> Result<()> {
        bail!("no callback expected")
    }
    fn respond_error(&self, _: &Value, _: i64, _: &str) -> Result<()> {
        bail!("no callback expected")
    }
}

struct Client(CommandHandle);

impl BackendClient for Client {
    fn kind(&self) -> BackendKind {
        BackendKind::Claude
    }
    fn capture_commands(&self) -> Result<CommandHandle> {
        Ok(self.0.clone())
    }
    fn call(&self, method: &str, params: Value, timeout: Duration) -> Result<String> {
        self.0.call(method, params, timeout)
    }
    fn respond(&self, _: &Value, _: Value) -> Result<()> {
        bail!("no callback expected")
    }
    fn respond_error(&self, _: &Value, _: i64, _: &str) -> Result<()> {
        bail!("no callback expected")
    }
    fn take_events_rx(&mut self) -> Result<EventReceiver> {
        bail!("test transfers its event receiver separately")
    }
    fn stop(&mut self) {}
}

#[test]
fn an_optional_quota_writer_cannot_block_snapshot_or_task_reads() {
    let (started, requests) = mpsc::channel();
    let gate = Arc::new(Gate::default());
    let client = Client(CommandHandle::new(Commands {
        gate: gate.clone(),
        started,
    }));
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.enable_claude();
    session.set_client_identity(carlos_proto::session::Backend::Claude, "test".into());
    let (_events, receiver) = mpsc::channel();
    let (engine, _notices) = EngineHandle::spawn(session, Box::new(client), receiver.into());
    let params = requests
        .recv_timeout(TIMEOUT)
        .expect("worker entered native quota writer");
    assert!(QuotaReadKey::from_context(&params).is_some());
    let started = Instant::now();
    for command in [
        EngineCommand::Snapshot { limit: 10 },
        EngineCommand::TaskSnapshot,
    ] {
        let reply = engine
            .request(command)
            .expect("bounded read admission")
            .recv_timeout(Duration::from_secs(1))
            .expect("owner remains readable during native write");
        assert!(matches!(
            reply,
            EngineReply::Snapshot(_) | EngineReply::TaskSnapshot(_)
        ));
    }
    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(
        requests.try_recv().is_err(),
        "no optional retry while the original write waits"
    );
    gate.release();
    engine.shutdown();
    engine.join().expect("owner stops after writer release");
}

struct Fixture(Owner);

impl Drop for Fixture {
    fn drop(&mut self) {
        self.0.channels.stop.store(true, Ordering::SeqCst);
        self.0.native.stop();
        self.0.workers.stop();
    }
}

fn fixture() -> Fixture {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.enable_claude();
    session.set_client_identity(carlos_proto::session::Backend::Claude, "test".into());
    let channels = Channels {
        final_read: Default::default(),
        ordinary: mpsc::channel().1,
        control: mpsc::channel().1,
        reads: mpsc::channel().1,
        wake: mpsc::sync_channel(1).0,
        wake_rx: mpsc::channel().1,
        notice: mpsc::sync_channel(1).0,
        stop: Arc::default(),
        changed: Arc::default(),
        goal_interest: Arc::default(),
        task_interest: Arc::default(),
    };
    Fixture(Owner::new(
        session,
        Box::<TestBackend>::default(),
        mpsc::channel().1.into(),
        channels,
    ))
}

#[test]
fn a_full_action_backlog_skips_optional_quota_without_stopping_the_session_or_busy_retrying() {
    let mut fixture = fixture();
    for _ in 0..64 {
        fixture.0.action(ServerRequestAction::RefreshContext);
    }
    let now = Instant::now();
    let params = fixture
        .0
        .session
        .prepare_claude_quota(now)
        .expect("scheduled read");
    let key = QuotaReadKey::from_context(&params).expect("key");
    fixture.0.quota_action(params);
    assert!(!fixture.0.session.claude_quota_request_current(key));
    assert!(fixture
        .0
        .session
        .prepare_claude_quota(now + Duration::from_secs(299))
        .is_none());
    assert!(!fixture.0.session.events.backend_closed);
    assert!(!fixture.0.channels.stop.load(Ordering::SeqCst));
}

#[test]
fn a_queued_quota_read_is_retired_before_dispatch_into_a_replacement_helper() {
    let mut fixture = fixture();
    let params = fixture
        .0
        .session
        .prepare_claude_quota(Instant::now())
        .expect("scheduled read");
    let key = QuotaReadKey::from_context(&params).expect("key");
    fixture.0.quota_action(params);
    fixture.0.session.set_claude_generation_for_test(2);
    fixture.0.dispatch_actions();
    assert_eq!(
        fixture.0.workers.actions_busy, 0,
        "stale quota never reaches native writer"
    );
    assert!(!fixture.0.session.claude_quota_request_current(key));
    assert!(!fixture.0.session.events.backend_closed);
}
