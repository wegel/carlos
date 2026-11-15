//! Interrupt an exact callback query without retiring its answer card or borrowing global focus.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{bail, Result};
use carlos_proto::commands::ApprovalAnswer;
use carlos_proto::session::Decision;
use serde_json::{json, Value};

use super::entry_id::EntryId;
use super::state::SessionState;
use crate::backend::BackendKind;
use crate::backend_commands::{BackendCommands, CommandHandle};
use crate::dispatch_guard::DispatchGuard;

type Calls = Arc<Mutex<Vec<(Value, Value)>>>;

#[derive(Clone)]
struct Endpoint {
    permission: DispatchGuard,
    calls: Calls,
    fail: bool,
}

impl BackendCommands for Endpoint {
    fn kind(&self) -> BackendKind {
        BackendKind::Codex
    }
    fn guarded(&self, permission: DispatchGuard) -> Result<CommandHandle> {
        let mut endpoint = self.clone();
        endpoint.permission = permission;
        Ok(CommandHandle::new(endpoint))
    }
    fn call(&self, method: &str, params: Value, _: Duration) -> Result<String> {
        self.permission.check()?;
        self.calls
            .lock()
            .expect("calls")
            .push((json!(method), params));
        if self.fail {
            bail!("original native query interrupt failed")
        }
        Ok(json!({"result":{}}).to_string())
    }
    fn respond(&self, id: &Value, result: Value) -> Result<()> {
        self.permission.check()?;
        self.calls.lock().expect("calls").push((id.clone(), result));
        Ok(())
    }
    fn respond_error(&self, _: &Value, _: i64, _: &str) -> Result<()> {
        bail!("unexpected error response")
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

fn question(
    state: &mut SessionState,
    id: Value,
    thread: &str,
    turn: Value,
    blocking: bool,
) -> EntryId {
    let frame = json!({"id":id,"params":{"threadId":thread,"turnId":turn,"itemId":"query-item","isBlocking":blocking,
        "questions":[{"id":"native-field","header":"Native header","question":"Exact native question?","options":[],"isOther":true}]}});
    state.accept_codex_callback("item/tool/requestUserInput", &frame);
    state
        .approval()
        .live()
        .find(|card| card.facts().request_id == id)
        .expect("original card")
        .entry_id()
}

fn interrupt() -> ApprovalAnswer {
    ApprovalAnswer::Decision {
        decision: Decision::Cancel,
    }
}

#[test]
fn query_interrupt_keeps_original_draft_card_and_focus_after_native_ack() {
    let mut state = SessionState::new("root".into(), Default::default(), false);
    state.enable_codex();
    let root = question(&mut state, json!(9), "root", json!("root-run"), true);
    let child = question(&mut state, json!("9"), "child", json!("child-run"), false);
    let (client, calls) = endpoint(false);
    let work = state
        .prepare_engine_answer(&client, child, interrupt())
        .expect("exact child interrupt");
    assert!(state
        .prepare_engine_answer(&client, child, interrupt())
        .is_err());
    state
        .finish_engine_control(&client, work.execute(&client))
        .expect("interrupt acknowledged");
    assert_eq!(
        *calls.lock().expect("calls"),
        [(
            json!("turn/interrupt"),
            json!({"threadId":"child","turnId":"child-run"})
        )]
    );
    assert_eq!(
        state
            .approval()
            .pending()
            .expect("root focus untouched")
            .entry_id(),
        root
    );
    let live = state
        .approval()
        .live()
        .find(|card| card.entry_id() == child)
        .expect("child still pending");
    assert!(!live.submitted());
    assert!(state.events.decisions.is_empty());
    assert!(state
        .prepare_engine_answer(&client, child, interrupt())
        .is_err());
    let work = state.prepare_engine_answer(&client, child, ApprovalAnswer::Input {
        value:json!({"fields":[{"key":"native-field","values":[],"text":"  kept 日本語  "}]}),
    }).expect("separate explicit original answer");
    state
        .finish_engine_control(&client, work.execute(&client))
        .expect("answer accepted");
    assert_eq!(
        calls.lock().expect("calls")[1],
        (
            json!("9"),
            json!({"answers":{"native-field":{"answers":["user_note: kept 日本語"]}}})
        )
    );
}

#[test]
fn failed_query_interrupt_reopens_only_its_original_live_claim() {
    let mut state = SessionState::new("root".into(), Default::default(), false);
    state.enable_codex();
    let card = question(
        &mut state,
        json!("child-query"),
        "child",
        json!("child-run"),
        true,
    );
    let (client, _) = endpoint(true);
    let work = state
        .prepare_engine_answer(&client, card, interrupt())
        .expect("claimed query");
    assert!(state
        .finish_engine_control(&client, work.execute(&client))
        .is_err());
    assert!(!state
        .approval()
        .live()
        .find(|live| live.entry_id() == card)
        .expect("same live query")
        .submitted());
    assert!(state
        .prepare_engine_answer(&client, card, interrupt())
        .is_ok());
    assert!(state.events.decisions.is_empty());
}

#[test]
fn retired_query_cannot_write_through_a_captured_control_or_reopen_a_replacement() {
    let mut state = SessionState::new("root".into(), Default::default(), false);
    state.enable_codex();
    let card = question(
        &mut state,
        json!("child-query"),
        "child",
        json!("old-run"),
        true,
    );
    let (client, calls) = endpoint(false);
    let work = state
        .prepare_engine_answer(&client, card, interrupt())
        .expect("captured query");
    state.retire_codex_callback(&json!("child-query"));
    let replacement = question(
        &mut state,
        json!("replacement"),
        "child",
        json!("new-run"),
        true,
    );
    assert!(state
        .finish_engine_control(&client, work.execute(&client))
        .is_err());
    assert!(calls.lock().expect("no native write").is_empty());
    assert_eq!(state.approval().live().count(), 1);
    assert_eq!(
        state.approval().pending().expect("new card").entry_id(),
        replacement
    );
    assert!(!state
        .approval()
        .pending()
        .expect("new card unclaimed")
        .submitted());
}

#[test]
fn malformed_question_without_original_turn_never_creates_an_interrupt_target() {
    let mut state = SessionState::new("root".into(), Default::default(), false);
    state.enable_codex();
    let params = json!({"threadId":"child","turnId":null,"itemId":"query-item",
        "questions":[{"id":"native-field","header":"Header","question":"Query?","options":[]}]});
    assert!(super::control_answers::question_interrupt_params(&params).is_none());
    state.accept_codex_callback(
        "item/tool/requestUserInput",
        &json!({"id":"no-turn","params":params}),
    );
    assert_eq!(state.approval().live().count(), 0);
    assert!(state.export_approvals().is_empty());
}
