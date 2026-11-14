//! Race card claims and reject stale or invalid priority controls before native I/O.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{bail, Result};
use carlos_proto::commands::ApprovalAnswer;
use carlos_proto::session::Decision;
use serde_json::{json, Value};

use super::approval_state::ApprovalQueue;
use super::control_work::ControlFailure;
use super::entry_id::EntryId;
use super::state::SessionState;
use crate::backend::BackendKind;
use crate::backend_commands::{BackendCommands, CommandHandle};

type Calls = Arc<Mutex<Vec<(Value, Value)>>>;
#[derive(Clone)]
struct Endpoint {
    permission: crate::dispatch_guard::DispatchGuard,
    calls: Calls,
    fail: bool,
}

impl BackendCommands for Endpoint {
    fn guarded(&self, permission: crate::dispatch_guard::DispatchGuard) -> Result<CommandHandle> {
        let mut endpoint = self.clone();
        endpoint.permission = permission;
        Ok(CommandHandle::new(endpoint))
    }

    fn kind(&self) -> BackendKind {
        BackendKind::Claude
    }
    fn call(&self, method: &str, params: Value, _: Duration) -> Result<String> {
        self.permission.check()?;
        self.calls
            .lock()
            .expect("calls")
            .push((json!(method), params));
        Ok(json!({"result":{}}).to_string())
    }
    fn respond(&self, id: &Value, result: Value) -> Result<()> {
        self.permission.check()?;
        self.calls.lock().expect("calls").push((id.clone(), result));
        if self.fail {
            bail!("reply lost after write");
        }
        Ok(())
    }
    fn respond_error(&self, _: &Value, _: i64, _: &str) -> Result<()> {
        bail!("unexpected error reply")
    }
}

fn endpoint(fail: bool) -> (CommandHandle, Calls) {
    let calls = Calls::default();
    (
        CommandHandle::new(Endpoint {
            permission: Default::default(),
            calls: calls.clone(),
            fail,
        }),
        calls,
    )
}

fn state() -> SessionState {
    SessionState::new("root".into(), Default::default(), false)
}
fn allow() -> ApprovalAnswer {
    ApprovalAnswer::Decision {
        decision: Decision::Once,
    }
}
fn card(state: &mut SessionState, native: &str) -> EntryId {
    state.receive_claude_callback(&json!({"callbackId":native,"kind":"permission","payload":{"toolName":"Read","input":{"file_path":"README.md"}}}));
    state
        .approval()
        .live()
        .find(|card| card.facts().request_id == native)
        .expect("live callback")
        .entry_id()
}

#[test]
fn only_one_client_can_claim_a_card_and_queued_cards_need_no_focus_switch() {
    let mut state = state();
    let first = card(&mut state, "first");
    let second = card(&mut state, "second");
    let (client, calls) = endpoint(false);
    let work = state
        .prepare_engine_answer(BackendKind::Claude, second, allow())
        .expect("claim queued card");
    assert!(state
        .prepare_engine_answer(BackendKind::Claude, second, allow())
        .is_err());
    assert_eq!(
        state
            .approval()
            .pending()
            .expect("focus unchanged")
            .entry_id(),
        first
    );
    state
        .finish_engine_control(&client, work.execute(&client))
        .expect("settle");
    assert_eq!(
        calls.lock().expect("calls").as_slice(),
        &[(json!("second"), json!({"behavior":"allow"}))]
    );
    assert_eq!(
        state
            .approval()
            .pending()
            .expect("first remains")
            .entry_id(),
        first
    );
    assert!(state.approval().queued(ApprovalQueue::Claude).is_empty());
}

#[test]
fn a_validation_error_reopens_only_its_card_without_native_io() {
    let mut state = state();
    let id = card(&mut state, "first");
    let (client, calls) = endpoint(false);
    let work = state
        .prepare_engine_answer(
            BackendKind::Claude,
            id,
            ApprovalAnswer::Decision {
                decision: Decision::Session,
            },
        )
        .expect("claim before validation");
    assert!(matches!(
        state.finish_engine_control(&client, work.execute(&client)),
        Err(ControlFailure::Invalid(_))
    ));
    assert!(calls.lock().expect("calls").is_empty());
    assert!(state
        .prepare_engine_answer(BackendKind::Claude, id, allow())
        .is_ok());
}

#[test]
fn an_uncertain_native_answer_keeps_the_card_claimed() {
    let mut state = state();
    let id = card(&mut state, "first");
    let (client, calls) = endpoint(true);
    let work = state
        .prepare_engine_answer(BackendKind::Claude, id, allow())
        .expect("claim");
    assert!(matches!(
        state.finish_engine_control(&client, work.execute(&client)),
        Err(ControlFailure::Native(_))
    ));
    assert_eq!(calls.lock().expect("calls").len(), 1);
    assert!(state
        .prepare_engine_answer(BackendKind::Claude, id, allow())
        .is_err());
}

#[test]
fn withdrawing_a_card_cancels_its_undispatched_answer() {
    let mut state = state();
    let id = card(&mut state, "first");
    let (client, calls) = endpoint(false);
    let work = state
        .prepare_engine_answer(BackendKind::Claude, id, allow())
        .expect("claim");
    let _ = state.retire_approval(id);
    assert!(matches!(
        state.finish_engine_control(&client, work.execute(&client)),
        Err(ControlFailure::Retired)
    ));
    assert!(calls.lock().expect("calls").is_empty());
}

#[test]
fn an_in_flight_interrupt_claim_reopens_only_when_its_own_completion_settles() {
    let mut state = state();
    let (client, calls) = endpoint(false);
    state.set_active_turn_id(Some("live".into()));
    let turn = state
        .publish()
        .snapshot(100)
        .info
        .current_turn
        .expect("turn token");
    let first = state
        .prepare_engine_interrupt(&client, &turn)
        .expect("first stop intent");
    assert!(state.prepare_engine_interrupt(&client, &turn).is_err());
    state
        .finish_engine_control(&client, first.execute(&client))
        .expect("native acknowledgement");
    let next = state
        .prepare_engine_interrupt(&client, &turn)
        .expect("acknowledged stop may be requested again");
    state.set_active_turn_id(Some("replacement".into()));
    let replacement = state
        .publish()
        .snapshot(100)
        .info
        .current_turn
        .expect("replacement token");
    let pending = state
        .prepare_engine_interrupt(&client, &replacement)
        .expect("new stop intent");
    let _ = state.finish_engine_control(&client, next.execute(&client));
    assert!(
        state
            .prepare_engine_interrupt(&client, &replacement)
            .is_err(),
        "old completion cannot release the new turn's claim"
    );
    state
        .finish_engine_control(&client, pending.execute(&client))
        .expect("new stop settles");
    assert_eq!(calls.lock().expect("native calls").len(), 2);
}

#[test]
fn a_changed_turn_or_dropped_owner_cancels_a_prepared_interrupt() {
    for drop_owner in [false, true] {
        let mut state = state();
        let (client, calls) = endpoint(false);
        state.set_active_turn_id(Some("old-turn".into()));
        let turn = state
            .publish()
            .snapshot(100)
            .info
            .current_turn
            .expect("public token");
        let work = state
            .prepare_engine_interrupt(&client, &turn)
            .expect("exact target");
        if drop_owner {
            drop(state);
        } else {
            state.set_active_turn_id(Some("new-turn".into()));
            assert_ne!(state.publish().snapshot(100).info.current_turn, Some(turn));
        }
        let _completion = work.execute(&client);
        assert!(calls.lock().expect("calls").is_empty());
    }
}

#[test]
fn a_control_completion_cannot_mutate_another_session() {
    let mut original = state();
    let id = card(&mut original, "first");
    let (client, _calls) = endpoint(false);
    let work = original
        .prepare_engine_answer(BackendKind::Claude, id, allow())
        .expect("claim");
    let mut other = state();
    other.set_status("other session");
    assert!(matches!(
        other.finish_engine_control(&client, work.execute(&client)),
        Err(ControlFailure::Retired)
    ));
    assert!(other.messages().is_empty());
    assert_eq!(other.status(), "other session");
    assert!(original
        .approval()
        .pending()
        .expect("original remains claimed")
        .submitted());
}

#[test]
fn structured_question_answers_validate_card_keys_values_and_required_fields() {
    let mut state = state();
    state.receive_claude_callback(&json!({"callbackId":"ask","kind":"permission","payload":{"toolName":"AskUserQuestion","input":{
        "questions":[{"question":"Pick one","header":"Choice","multiSelect":false,"options":[{"label":"A"},{"label":"B"}]}]}}}));
    let id = state.approval().pending().expect("question").entry_id();
    let (client, calls) = endpoint(false);
    for input in [
        json!({"fields":[]}),
        json!({"fields":[{"key":"foreign","values":["A"],"text":null}]}),
        json!({"fields":[{"key":"Pick one","values":["invented"],"text":null}]}),
        json!({"fields":[{"key":"Pick one","values":["A","B"],"text":null}]}),
    ] {
        let work = state
            .prepare_engine_answer(
                BackendKind::Claude,
                id,
                ApprovalAnswer::Input { value: input },
            )
            .expect("claim");
        assert!(matches!(
            state.finish_engine_control(&client, work.execute(&client)),
            Err(ControlFailure::Invalid(_))
        ));
    }
    assert!(calls.lock().expect("calls").is_empty());
    let work = state
        .prepare_engine_answer(
            BackendKind::Claude,
            id,
            ApprovalAnswer::Input {
                value: json!({"fields":[{"key":"Pick one","values":[],"text":"日本語"}]}),
            },
        )
        .expect("claim valid input");
    state
        .finish_engine_control(&client, work.execute(&client))
        .expect("valid native answer");
    assert_eq!(
        calls.lock().expect("calls")[0].1["updatedInput"]["answers"]["Pick one"],
        "日本語"
    );
}

#[test]
fn answering_the_active_card_promotes_the_next_without_leaving_an_idle_gap() {
    let mut state = state();
    let first = card(&mut state, "first");
    let second = card(&mut state, "second");
    let (client, _calls) = endpoint(false);
    let work = state
        .prepare_engine_answer(BackendKind::Claude, first, allow())
        .expect("claim active");
    state
        .finish_engine_control(&client, work.execute(&client))
        .expect("answer active");
    let snapshot = state.publish().snapshot(100);
    assert_eq!(snapshot.approvals.len(), 1);
    assert_eq!(snapshot.approvals[0].id, second.to_wire());
    assert!(snapshot.approvals[0].active);
    assert_eq!(snapshot.info.phase, carlos_proto::session::Phase::Approval);
}

#[test]
fn an_approval_uses_no_ordinary_mutation_slot() {
    let mut state = state();
    let id = card(&mut state, "first");
    let ordinary = state
        .prepare_turn_text(
            BackendKind::Claude,
            "pending send".into(),
            super::prompt_commands::PromptOptions {
                record_history: true,
                force_start: false,
                origin: crate::backend::PromptOrigin::Human,
            },
            &super::prompt_commands::PromptContext {
                operation: None,
                claude_queue: false,
            },
        )
        .expect("prepare send")
        .work
        .expect("work");
    let (client, calls) = endpoint(false);
    let control = state
        .prepare_engine_answer(BackendKind::Claude, id, allow())
        .expect("control during send");
    state
        .finish_engine_control(&client, control.execute(&client))
        .expect("control settles independently");
    assert!(state
        .prepare_rename(BackendKind::Claude, "still busy")
        .is_err());
    assert_eq!(calls.lock().expect("control only").len(), 1);
    state
        .finish_backend_work(ordinary.execute(&client))
        .expect("ordinary settles separately");
    assert!(state.prepare_rename(BackendKind::Claude, "free").is_ok());
}
