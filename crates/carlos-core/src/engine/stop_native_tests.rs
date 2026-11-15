//! Stop the selected attached conversation through its captured writer while another owner stays live.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::time::Duration;

use anyhow::{bail, ensure, Result};
use carlos_proto::session::Availability;
use serde_json::{json, Value};

use super::{EngineCommand, EngineHandle, EngineReply};
use crate::backend::{BackendClient, BackendKind, EventReceiver};
use crate::backend_commands::{BackendCommands, CommandHandle};
use crate::dispatch_guard::DispatchGuard;
use crate::session::state::SessionState;

type Calls = Arc<Mutex<Vec<(String, Value, Duration)>>>;

#[derive(Clone)]
struct Endpoint {
    calls: Calls,
    closed: Arc<AtomicBool>,
    permission: DispatchGuard,
    fail: bool,
    remote_active: bool,
    gate: Option<Arc<Gate>>,
}

impl BackendCommands for Endpoint {
    fn kind(&self) -> BackendKind {
        BackendKind::Codex
    }
    fn connection_generation(&self) -> Option<u64> {
        Some(1)
    }
    fn guarded(&self, permission: DispatchGuard) -> Result<CommandHandle> {
        Ok(CommandHandle::new(Self {
            permission,
            ..self.clone()
        }))
    }
    fn call(&self, method: &str, params: Value, timeout: Duration) -> Result<String> {
        self.permission.check()?;
        ensure!(
            !self.closed.load(Ordering::SeqCst),
            "connection closed before interrupt"
        );
        self.calls
            .lock()
            .expect("native calls")
            .push((method.into(), params.clone(), timeout));
        if let Some(gate) = self.gate.as_ref().filter(|_| method == "turn/interrupt") {
            gate.wait(timeout)?;
        }
        if self.fail {
            bail!("native acknowledgement lost");
        }
        if method == "thread/read" {
            return Ok(json!({"result":{"thread":{"id":params["threadId"],"status":{"type":if self.remote_active {"active"} else {"idle"}}}}}).to_string());
        }
        Ok(json!({"result":{}}).to_string())
    }
    fn respond(&self, _: &Value, _: Value) -> Result<()> {
        bail!("unexpected answer")
    }
    fn respond_error(&self, _: &Value, _: i64, _: &str) -> Result<()> {
        bail!("unexpected answer")
    }
}

impl BackendClient for Endpoint {
    fn kind(&self) -> BackendKind {
        BackendKind::Codex
    }
    fn connection_generation(&self) -> Option<u64> {
        Some(1)
    }
    fn capture_commands(&self) -> Result<CommandHandle> {
        Ok(CommandHandle::new(self.clone()))
    }
    fn call(&self, _: &str, _: Value, _: Duration) -> Result<String> {
        bail!("uncaptured native call")
    }
    fn respond(&self, _: &Value, _: Value) -> Result<()> {
        bail!("uncaptured native answer")
    }
    fn respond_error(&self, _: &Value, _: i64, _: &str) -> Result<()> {
        bail!("uncaptured native answer")
    }
    fn take_events_rx(&mut self) -> Result<EventReceiver> {
        bail!("fixture supplies events separately")
    }
    fn stop(&mut self) {
        self.closed.store(true, Ordering::SeqCst);
    }
}

struct Fixture {
    owner: EngineHandle,
    _events: mpsc::Sender<String>,
    closed: Arc<AtomicBool>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.owner.shutdown();
        self.owner.join().expect("test owner joins");
    }
}

fn fixture(thread: &str, calls: Calls, fail: bool) -> Fixture {
    fixture_state(thread, calls, fail, true, false)
}

fn fixture_state(
    thread: &str,
    calls: Calls,
    fail: bool,
    known_turn: bool,
    remote_active: bool,
) -> Fixture {
    fixture_control(thread, calls, fail, known_turn, remote_active, None)
}

fn fixture_control(
    thread: &str,
    calls: Calls,
    fail: bool,
    known_turn: bool,
    remote_active: bool,
    gate: Option<Arc<Gate>>,
) -> Fixture {
    let closed = Arc::new(AtomicBool::new(false));
    let backend = Endpoint {
        calls,
        closed: closed.clone(),
        permission: Default::default(),
        fail,
        remote_active,
        gate,
    };
    let mut session = SessionState::new(thread.into(), Default::default(), false);
    session.enable_codex();
    session.configure_codex_connection(1, Some("/tmp/recorded-attached-server.sock".into()));
    if known_turn {
        session.set_active_turn_id(Some(format!("{thread}-turn")));
    }
    let (events, receiver) = mpsc::channel();
    let (owner, _) = EngineHandle::spawn(session, Box::new(backend), receiver.into());
    Fixture {
        owner,
        _events: events,
        closed,
    }
}

#[test]
fn explicit_stop_interrupts_only_the_selected_attached_turn_before_closing() {
    let calls = Calls::default();
    let selected = fixture("selected", calls.clone(), false);
    let other = fixture("bystander", calls.clone(), false);
    let reply = selected
        .owner
        .stop_session()
        .expect("explicit stop")
        .recv_timeout(Duration::from_secs(5))
        .expect("stop finishes");
    let EngineReply::Stopped { update, native } = reply else {
        panic!("stop report");
    };
    native.expect("captured interrupt acknowledged");
    selected.owner.join().expect("selected owner joined");
    assert_eq!(update.info.availability, Availability::Closed);
    assert!(selected.closed.load(Ordering::SeqCst));
    assert!(!other.closed.load(Ordering::SeqCst));
    let info = other
        .owner
        .request(EngineCommand::Info)
        .expect("other owner remains readable")
        .recv_timeout(Duration::from_secs(1))
        .expect("other owner stays responsive");
    assert!(
        matches!(info, EngineReply::Info { info, .. } if info.availability == Availability::Ready)
    );
    assert_eq!(
        calls.lock().expect("calls").as_slice(),
        &[(
            "turn/interrupt".into(),
            json!({"threadId":"selected","turnId":"selected-turn"}),
            Duration::from_secs(2)
        )]
    );
}

#[test]
fn failed_native_interrupt_remains_uncertain_even_after_the_owner_is_released() {
    let calls = Calls::default();
    let selected = fixture("selected", calls.clone(), true);
    let reply = selected
        .owner
        .stop_session()
        .expect("explicit stop")
        .recv_timeout(Duration::from_secs(5))
        .expect("local owner still closes");
    let EngineReply::Stopped { update, native } = reply else {
        panic!("stop report");
    };
    let error = native.expect_err("closing a connection is not an interrupt acknowledgement");
    assert_eq!(error.code, -32012);
    assert!(error.message.contains("acknowledgement lost"));
    selected.owner.join().expect("closed owner joins");
    assert_eq!(update.info.availability, Availability::Closed);
    assert!(selected.closed.load(Ordering::SeqCst));
    assert_eq!(calls.lock().expect("one original write").len(), 1);
}

#[test]
fn normal_shutdown_does_not_add_an_interrupt_to_the_attached_server() {
    let calls = Calls::default();
    let selected = fixture("selected", calls.clone(), false);
    selected.owner.shutdown();
    selected.owner.join().expect("normal daemon teardown");
    assert!(selected.closed.load(Ordering::SeqCst));
    assert!(calls.lock().expect("calls").is_empty());
}

#[test]
fn a_missing_local_turn_is_checked_against_native_idle_before_detaching() {
    let calls = Calls::default();
    let selected = fixture_state("selected", calls.clone(), false, false, false);
    let reply = selected
        .owner
        .stop_session()
        .expect("explicit stop")
        .recv_timeout(Duration::from_secs(5))
        .expect("stop finishes");
    let EngineReply::Stopped { native, .. } = reply else {
        panic!("stop report");
    };
    native.expect("native thread confirms idle");
    selected.owner.join().expect("owner closes");
    assert_eq!(
        calls.lock().expect("one read").as_slice(),
        &[(
            "thread/read".into(),
            json!({"threadId":"selected","includeTurns":false}),
            Duration::from_secs(2)
        )]
    );
}

#[test]
fn native_activity_without_a_safe_turn_id_stays_uncertain_and_is_never_interrupted_blindly() {
    let calls = Calls::default();
    let selected = fixture_state("selected", calls.clone(), false, false, true);
    let reply = selected
        .owner
        .stop_session()
        .expect("explicit stop")
        .recv_timeout(Duration::from_secs(5))
        .expect("local owner closes");
    let EngineReply::Stopped { native, .. } = reply else {
        panic!("stop report");
    };
    assert!(native
        .expect_err("active without target")
        .message
        .contains("no safe turn identifier"));
    selected
        .owner
        .join()
        .expect("owner closes despite uncertain remote work");
    let requests = calls.lock().expect("native calls");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].0, "thread/read");
}

struct Gate {
    started: mpsc::SyncSender<()>,
    released: Mutex<bool>,
    wake: Condvar,
}

impl Gate {
    fn wait(&self, timeout: Duration) -> Result<()> {
        let _ = self.started.try_send(());
        let (released, _) = self
            .wake
            .wait_timeout_while(
                self.released.lock().expect("release gate"),
                timeout,
                |released| !*released,
            )
            .expect("wait for native acknowledgement");
        ensure!(*released, "fixture native acknowledgement timed out");
        Ok(())
    }

    fn release(&self) {
        *self.released.lock().expect("release gate") = true;
        self.wake.notify_all();
    }
}

#[test]
fn a_second_stop_cannot_claim_a_request_while_the_first_native_acknowledgement_waits() {
    let (started, entered) = mpsc::sync_channel(1);
    let gate = Arc::new(Gate {
        started,
        released: Mutex::new(false),
        wake: Condvar::new(),
    });
    let calls = Calls::default();
    let selected = fixture_control(
        "selected",
        calls.clone(),
        false,
        true,
        false,
        Some(gate.clone()),
    );
    let stopped = selected.owner.stop_session().expect("first explicit stop");
    entered
        .recv_timeout(Duration::from_secs(1))
        .expect("owner consumed the first request");
    assert!(
        selected.owner.stop_session().is_err(),
        "a second request must not be accepted after the owner took the first reply slot"
    );
    gate.release();
    let reply = stopped
        .recv_timeout(Duration::from_secs(5))
        .expect("original stop reply");
    assert!(matches!(reply, EngineReply::Stopped { native: Ok(()), .. }));
    selected.owner.join().expect("one owner cleanup");
    assert_eq!(calls.lock().expect("one native interrupt").len(), 1);
}
