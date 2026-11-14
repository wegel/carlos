//! Keep write uncertainty and early native acknowledgements ordered under one private operation.

use std::sync::{mpsc, Arc};
use std::time::Instant;

use carlos_proto::submissions::{ClientOperation, OperationState};
use carlos_proto::tasks::TaskKind;
use serde_json::json;

use super::{PendingTaskStop, Target};
use crate::backend::BackendKind;
use crate::engine::mailbox::Mailbox;
use crate::engine::owner::{Channels, Owner};
use crate::engine::EngineCommand;
use crate::session::state::SessionState;
use crate::session::task_stop_reply::TaskStopReply;
use crate::session::test_backend::TestBackend;

struct Fixture(Owner);

impl Drop for Fixture {
    fn drop(&mut self) {
        self.0
            .channels
            .stop
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.0.native.stop();
        self.0.workers.stop();
    }
}

fn fixture() -> (Fixture, ClientOperation) {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.enable_codex();
    session.set_codex_agents_for_test(vec![
        json!({"id":"child","lastObservedTurn":{"id":"turn","status":"inProgress"}}),
    ]);
    session.verify_codex_agent_owners_for_test(session.codex_scope().epoch());
    let channels = Channels {
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
    let mut owner = Owner::new(
        session,
        Box::<TestBackend>::default(),
        mpsc::channel().1.into(),
        channels,
    );
    let (mailbox, receiver) = Mailbox::new(1, 1024);
    let _reply = mailbox
        .request(EngineCommand::Info)
        .expect("admission permit");
    let operation: ClientOperation =
        serde_json::from_value(json!({"client":"a".repeat(32),"id":"1"})).expect("operation");
    owner.results.admit(
        owner.session.session_id(),
        operation.clone(),
        receiver.recv().expect("permit").permit,
    );
    let task = owner.session.task_page(None, None, 1).expect("task").tasks[0].clone();
    let work = owner
        .session
        .prepare_task_stop(operation.clone(), task.id, task.run.expect("run"))
        .expect("prepared target");
    let target = Target::capture(&owner.session, BackendKind::Codex);
    owner.task_stops.insert(
        operation.clone(),
        PendingTaskStop {
            work,
            target,
            started: Instant::now(),
        },
    );
    owner.workers.controls_busy = 1;
    (Fixture(owner), operation)
}

#[test]
fn a_task_stop_write_error_is_uncertain_without_a_native_acknowledgement() {
    let (mut fixture, operation) = fixture();
    fixture.0.finish_task_stop_dispatch(
        operation.clone(),
        Err("pipe timed out after a possible write".into()),
    );
    assert_eq!(fixture.0.workers.controls_busy, 0);
    let results = fixture.0.results.collect();
    assert_eq!(results[0].operation, operation);
    assert!(matches!(
        results[0].state,
        OperationState::Uncertain { recovery: None, .. }
    ));
    assert!(fixture.0.task_stops.is_empty());
}

#[test]
fn a_native_ack_before_writer_completion_cannot_be_overwritten_by_a_late_local_error() {
    let (mut fixture, operation) = fixture();
    let work = &fixture.0.task_stops[&operation].work;
    fixture.0.finish_task_stop(TaskStopReply {
        operation: operation.clone(),
        task: work.claim.task.clone(),
        run: work.claim.run.clone(),
        kind: TaskKind::Agent,
        state: OperationState::Accepted { entry: None },
    });
    fixture
        .0
        .finish_task_stop_dispatch(operation, Err("late local writer failure".into()));
    assert_eq!(fixture.0.workers.controls_busy, 0);
    assert!(matches!(
        fixture.0.results.collect()[0].state,
        OperationState::Accepted { .. }
    ));
}
