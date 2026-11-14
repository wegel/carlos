//! Check exact native stop identities before writes and after replacement acknowledgements.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use carlos_proto::ids::TaskId;
use carlos_proto::session::Backend;
use carlos_proto::submissions::ClientOperation;
use serde_json::{json, Value};

use super::codex::panel_state::Process;
use super::state::SessionState;
use super::task_stop_work::TaskStopWork;
use crate::backend::BackendKind;
use crate::backend_commands::{BackendCommands, CommandHandle};
use crate::dispatch_guard::DispatchGuard;

type Pending = Arc<Mutex<Option<(DispatchGuard, String, Value)>>>;

#[derive(Clone, Default)]
struct Writer(Pending);

impl BackendCommands for Writer {
    fn kind(&self) -> BackendKind {
        BackendKind::Codex
    }
    fn guarded(&self, permission: DispatchGuard) -> Result<CommandHandle> {
        *self.0.lock().expect("pending write") = Some((permission, String::new(), Value::Null));
        Ok(CommandHandle::new(self.clone()))
    }
    fn request_control_with_context(&self, method: &str, params: Value, _: Value) -> Result<()> {
        let mut pending = self.0.lock().expect("pending write");
        let (_, captured_method, captured_params) = pending.as_mut().expect("guarded handle");
        *captured_method = method.into();
        *captured_params = params;
        Ok(())
    }
    fn call(&self, _: &str, _: Value, _: Duration) -> Result<String> {
        unreachable!()
    }
    fn respond(&self, _: &Value, _: Value) -> Result<()> {
        unreachable!()
    }
    fn respond_error(&self, _: &Value, _: i64, _: &str) -> Result<()> {
        unreachable!()
    }
}

fn operation(counter: u64) -> ClientOperation {
    serde_json::from_value(json!({"client":"a".repeat(32),"id":counter.to_string()}))
        .expect("operation")
}

fn state() -> SessionState {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.enable_codex();
    session.set_codex_agents_for_test(vec![json!({"id":"child","parentThreadId":"root",
        "source":{"subAgent":{}},"status":{"type":"active"},
        "lastObservedTurn":{"id":"turn-a","status":"inProgress"}})]);
    session.verify_codex_agent_owners_for_test(session.codex_scope().epoch());
    session
}

fn stop(session: &mut SessionState, index: usize, counter: u64) -> TaskStopWork {
    let task = &session.task_page(None, None, 32).expect("task page").tasks[index];
    session
        .prepare_task_stop(
            operation(counter),
            task.id.clone(),
            task.run.clone().expect("observed run"),
        )
        .expect("checked exact native run")
}

fn process(session: &mut SessionState, id: &str, pid: u64) {
    session.codex_panel.processes.push(Process {
        thread: "root".into(),
        data: json!({"processId":id,"itemId":"item","osPid":pid}),
    });
}

#[test]
fn queued_native_stops_keep_exact_parameters_and_retire_before_a_root_or_epoch_change() {
    for change_epoch in [false, true] {
        let mut session = state();
        let work = stop(&mut session, 0, 1);
        let writer = Writer::default();
        work.dispatch(&CommandHandle::new(writer.clone()))
            .expect("enqueue only");
        let pending = writer
            .0
            .lock()
            .expect("queued write")
            .take()
            .expect("captured");
        assert_eq!(pending.1, "turn/interrupt");
        assert_eq!(pending.2, json!({"threadId":"child","turnId":"turn-a"}));
        pending.0.check().expect("original target live");
        if change_epoch {
            session.advance_codex_epoch();
        } else {
            session.adopt_native_thread("fork".into());
        }
        assert!(
            pending.0.check().is_err(),
            "writer cannot follow a replacement root"
        );
    }
}

#[test]
fn a_prepared_child_interrupt_keeps_its_original_turn_and_new_intents_recheck_the_run() {
    let mut session = state();
    let work = stop(&mut session, 0, 1);
    let scope = session.backend_context().panel;
    assert!(super::codex::panel_events::event(
        &mut session,
        scope,
        "turn/started",
        &json!({"threadId":"child","turn":{"id":"turn-b","status":"inProgress"}})
    ));
    assert!(work.claim.live.load(Ordering::SeqCst));
    assert_eq!(work.claim.target.request().1["turnId"], "turn-a");
    let task = session
        .task_page(None, None, 32)
        .expect("replacement")
        .tasks[0]
        .clone();
    assert!(session
        .prepare_task_stop(operation(2), task.id, work.claim.run.clone())
        .is_err());
    stop(&mut session, 0, 3);
}

#[test]
fn a_process_ack_never_removes_a_reused_id_and_uncertain_scopes_cannot_be_stopped() {
    let mut session = state();
    process(&mut session, "process", 11);
    let work = stop(&mut session, 1, 1);
    let params = work.claim.target.request();
    assert_eq!(params.0, "thread/backgroundTerminals/terminate");
    assert_eq!(params.1, json!({"threadId":"root","processId":"process"}));
    let mut replacement = vec![Process {
        thread: "root".into(),
        data: session.codex_panel.processes[0].data.clone(),
    }];
    replacement[0].data["osPid"] = json!(12);
    session
        .task_stops
        .reconcile_processes(Some("root"), &replacement);
    assert!(!work.claim.live.load(Ordering::SeqCst));
    session.codex_panel.processes = replacement;
    work.acknowledged(&mut session);
    assert_eq!(session.codex_panel.processes[0].data["osPid"], 12);
    let current = session.task_page(None, None, 32).expect("new PID").tasks[1].clone();
    session.codex_task_status.uncertain.insert("root".into());
    let page = session.task_page(None, None, 32).expect("unknown scope");
    assert_eq!(page.tasks[1].state, "unknown");
    assert!(page.tasks[1].run.is_none());
    assert!(session
        .prepare_task_stop(operation(2), current.id, current.run.expect("old run"))
        .is_err());
}

#[test]
fn pending_stops_are_bounded_and_only_the_matching_operation_releases_a_claim() {
    let mut session = state();
    for index in 0..9 {
        process(&mut session, &format!("p{index}"), index + 1);
    }
    let mut work = Vec::new();
    for index in 0..8 {
        work.push(stop(&mut session, index, index as u64 + 1));
    }
    let page = session.task_page(None, None, 32).expect("all tasks");
    let ninth = &page.tasks[8];
    assert!(session
        .prepare_task_stop(
            operation(10),
            ninth.id.clone(),
            ninth.run.clone().expect("run")
        )
        .is_err());
    session.task_stops.finish(&operation(100));
    assert!(work
        .iter()
        .all(|work| work.claim.live.load(Ordering::SeqCst)));
    session.task_stops.finish(&operation(1));
    assert!(!work[0].claim.live.load(Ordering::SeqCst));
    assert!(work[1].claim.live.load(Ordering::SeqCst));
    stop(&mut session, 8, 10);
    let bogus: TaskId = "0".repeat(64).parse().expect("opaque id");
    assert!(session
        .prepare_task_stop(operation(11), bogus, work[0].claim.run.clone())
        .is_err());
}

#[test]
fn claude_stops_keep_the_exact_helper_run_and_acknowledgements_do_not_complete_it() {
    let mut session = SessionState::new("claude".into(), Default::default(), false);
    session.set_client_identity(Backend::Claude, "host".into());
    session.enable_claude();
    session
        .update_claude_task(&json!({"subtype":"task_started","task_id":"task","run_id":"a"}))
        .expect("run a");
    let old = stop(&mut session, 0, 1);
    assert_eq!(
        old.claim.target.request().1,
        json!({"taskId":"task","runId":"a"})
    );
    session
        .update_claude_task(&json!({"subtype":"task_started","task_id":"task","run_id":"b"}))
        .expect("run b");
    assert!(old.claim.live.load(Ordering::SeqCst));
    let new = stop(&mut session, 0, 2);
    session.receive_claude_task_stop("task", Some("a"), Some("late failure".into()));
    assert!(new.claim.live.load(Ordering::SeqCst));
    session.receive_claude_task_stop("task", Some("b"), None);
    assert_eq!(
        session
            .task_page(None, None, 32)
            .expect("native state")
            .tasks[0]
            .state,
        "running"
    );
    session.task_stops.finish(&operation(2));
    let task = &session.task_page(None, None, 32).expect("same run").tasks[0];
    assert!(session
        .prepare_task_stop(
            operation(3),
            task.id.clone(),
            task.run.clone().expect("run")
        )
        .is_err());
}
