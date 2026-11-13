//! Keep the session readable while a worker waits, and fence replies after native facts advance.

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use anyhow::{bail, Result};
use serde_json::{json, Value};

use super::command_work::{BackendWork, CommandError};
use super::models::{Role, TurnStatus};
use super::prompt_commands::{PromptContext, PromptOptions};
use super::state::SessionState;
use crate::backend::{BackendClient, BackendKind, EventReceiver, PromptOrigin};

struct Peer {
    kind: BackendKind,
    entered: Sender<()>,
    release: Receiver<()>,
}

impl BackendClient for Peer {
    fn kind(&self) -> BackendKind {
        self.kind
    }
    fn call(&self, _: &str, _: Value, _: Duration) -> Result<String> {
        self.entered.send(()).expect("worker entered");
        self.release
            .recv_timeout(Duration::from_secs(5))
            .expect("release worker");
        Ok(json!({"result":{"turn":{"id":"accepted"}}}).to_string())
    }
    fn respond(&self, _: &Value, _: Value) -> Result<()> {
        bail!("unexpected reply")
    }
    fn respond_error(&self, _: &Value, _: i64, _: &str) -> Result<()> {
        bail!("unexpected error")
    }
    fn take_events_rx(&mut self) -> Result<EventReceiver> {
        bail!("no receiver")
    }
    fn stop(&mut self) {}
}

fn state() -> SessionState {
    SessionState::new("root".into(), Default::default(), false)
}

fn prepare(state: &mut SessionState, kind: BackendKind) -> BackendWork {
    state
        .prepare_turn_text(
            kind,
            "submitted".into(),
            PromptOptions {
                record_history: true,
                force_start: false,
                origin: PromptOrigin::Human,
            },
            &PromptContext {
                operation: None,
                claude_queue: false,
            },
        )
        .expect("prepare")
        .work
        .expect("work")
}

fn worker(
    work: BackendWork,
    kind: BackendKind,
) -> (
    Receiver<()>,
    Sender<()>,
    thread::JoinHandle<super::command_work::BackendCompletion>,
) {
    let (entered, observed) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let worker = thread::spawn(move || {
        work.execute_embedded(&Peer {
            kind,
            entered,
            release: released,
        })
    });
    (observed, release, worker)
}

#[test]
fn delayed_backend_leaves_owner_available_for_native_facts_and_another_client_read() {
    let mut state = state();
    let work = prepare(&mut state, BackendKind::Codex);
    let (entered, release, worker) = worker(work, BackendKind::Codex);
    entered
        .recv_timeout(Duration::from_secs(2))
        .expect("backend is waiting");
    state.queue_turn_input_with_history("queued human", true);
    assert!(state
        .take_dispatchable_prompt(BackendKind::Codex, true, std::time::Instant::now())
        .is_none());
    let update = state.handle_backend_line(
        &json!({"method":"item/agentMessage/delta",
        "params":{"threadId":"root","itemId":"live","delta":"fact during wait"}})
        .to_string(),
        &state.backend_context(),
    );
    drop(update);
    assert_eq!(
        state.publish().snapshot(100).entries[0].text,
        "fact during wait"
    );
    let busy = state.prepare_rename(BackendKind::Codex, "other");
    assert!(matches!(busy, Err(CommandError::Busy { .. })));
    release.send(()).expect("release");
    state
        .finish_backend_work(worker.join().expect("worker"))
        .expect("settle");
    assert_eq!(
        state
            .take_dispatchable_prompt(BackendKind::Codex, true, std::time::Instant::now())
            .expect("retained human")
            .text,
        "queued human"
    );
    assert!(state
        .prepare_rename(BackendKind::Codex, "after completion")
        .is_ok());
}

#[test]
fn a_fork_or_connection_change_rejects_completion_without_modifying_new_history() {
    let mut state = state();
    let work = prepare(&mut state, BackendKind::Claude);
    let (entered, release, worker) = worker(work, BackendKind::Claude);
    entered
        .recv_timeout(Duration::from_secs(2))
        .expect("waiting");
    state.adopt_native_thread("new-thread".into());
    state.reset_for_forked_thread();
    state.append_message(Role::User, "new history");
    release.send(()).expect("release");
    let error = state
        .finish_backend_work(worker.join().expect("worker"))
        .err()
        .expect("stale");
    assert!(matches!(error, CommandError::Stale { .. }));
    assert_eq!(
        error.into_submitted().expect("recoverable submission").text,
        "submitted"
    );
    assert_eq!(state.messages().len(), 1);
    assert_eq!(state.messages()[0].text, "new history");
    assert!(state.prepare_rename(BackendKind::Claude, "next").is_ok());
}

#[test]
fn consumed_and_completed_claude_prompt_before_reply_is_not_echoed_or_started_twice() {
    let mut state = state();
    let work = prepare(&mut state, BackendKind::Claude);
    let (entered, release, worker) = worker(work, BackendKind::Claude);
    entered
        .recv_timeout(Duration::from_secs(2))
        .expect("waiting");
    state
        .register_claude_prompt(&json!({"uuid":"accepted","text":"submitted","origin":"human"}))
        .expect("register");
    let key = state.backend_entry_key("claude-prompt:accepted");
    state.consume_claude_prompt("accepted", &key);
    assert!(state.record_turn_completed("accepted", TurnStatus::Completed));
    release.send(()).expect("release");
    state
        .finish_backend_work(worker.join().expect("worker"))
        .expect("settle");
    assert_eq!(state.messages().len(), 1);
    assert_eq!(state.messages()[0].text, "submitted");
    assert!(state.active_turn_id().is_none());
    assert_eq!(state.input_history().history_len(), 1);
}

#[test]
fn an_ack_after_completion_without_a_consumed_echo_cannot_resurrect_the_turn() {
    let mut state = state();
    let work = prepare(&mut state, BackendKind::Claude);
    let (entered, release, worker) = worker(work, BackendKind::Claude);
    entered
        .recv_timeout(Duration::from_secs(2))
        .expect("waiting");
    state.record_turn_started("accepted");
    state.record_turn_completed("accepted", TurnStatus::Completed);
    release.send(()).expect("release");
    state
        .finish_backend_work(worker.join().expect("worker"))
        .expect("settle");
    assert!(state.active_turn_id().is_none());
    assert_eq!(state.messages().len(), 1);
}

#[test]
fn a_late_steer_for_an_old_turn_returns_uncertainty_without_touching_the_new_turn() {
    let mut state = state();
    state.record_turn_started("old");
    let work = prepare(&mut state, BackendKind::Codex);
    let (entered, release, worker) = worker(work, BackendKind::Codex);
    entered
        .recv_timeout(Duration::from_secs(2))
        .expect("waiting");
    state.record_turn_completed("old", TurnStatus::Completed);
    state.record_turn_started("new");
    release.send(()).expect("release");
    assert!(matches!(
        state.finish_backend_work(worker.join().expect("worker")),
        Err(CommandError::Stale { .. })
    ));
    assert_eq!(state.active_turn_id().as_deref(), Some("new"));
    assert!(state.messages().is_empty());
}

#[test]
fn old_completion_cannot_clear_a_new_commands_slot_after_reset() {
    let mut state = state();
    let work = prepare(&mut state, BackendKind::Claude);
    let (entered, release, worker) = worker(work, BackendKind::Claude);
    entered
        .recv_timeout(Duration::from_secs(2))
        .expect("waiting");
    state.reset_for_forked_thread();
    let _next = prepare(&mut state, BackendKind::Claude);
    release.send(()).expect("release");
    assert!(matches!(
        state.finish_backend_work(worker.join().expect("worker")),
        Err(CommandError::Stale { .. })
    ));
    assert!(matches!(
        state.prepare_rename(BackendKind::Claude, "other"),
        Err(CommandError::Busy { .. })
    ));
}
