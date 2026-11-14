//! Keep child reads, native tokens, deadlines and cache budgets independent of any client's focus.

use std::time::{Duration, Instant};

use serde_json::json;

use super::panel;
use super::panel_receive;
use super::panel_scope::{PanelScope, PanelUpdate};
use super::panel_state::{PanelId, Request, MAX_BYTES};
use super::scope::CodexScope;
use crate::session::state::SessionState;

fn scope(selected: Option<&PanelId>) -> PanelScope<'_> {
    PanelScope {
        native: CodexScope::for_test(true, false, 0),
        generation: 1,
        rebuilding: false,
        visible: false,
        detail: true,
        selected,
    }
}

fn state() -> SessionState {
    let mut state = SessionState::new("root".into(), Default::default(), false);
    for index in 0..9 {
        state.push_codex_agent_for_test(
            json!({"id":format!("child-{index}"),"parentThreadId":"root"}),
        );
    }
    state
}

fn read(state: &mut SessionState, thread: &str) -> Request {
    panel::details(
        state,
        scope(Some(&PanelId::Agent(thread.into()))),
        &mut PanelUpdate::default(),
        false,
    );
    state.codex_panel.activities[thread]
        .request
        .clone()
        .expect("targeted read")
}

fn reply(state: &mut SessionState, request: &Request, text: &str) {
    let selected = PanelId::Agent("child-8".into());
    panel_receive::reply(
        state,
        scope(Some(&selected)),
        &mut PanelUpdate::default(),
        &json!({
            "requestContext":{"operation":"panel","epoch":0,"root":"root","token":request.token},
            "requestMethod":request.method,"requestParams":request.params,
            "response":{"result":{"data":[{"turnId":"turn","item":{"id":"item","type":"agentMessage","text":text}}],"nextCursor":null}}
        }),
    );
}

#[test]
fn two_clients_child_details_complete_out_of_order_without_using_the_selected_row() {
    let mut state = state();
    let first = read(&mut state, "child-0");
    let second = read(&mut state, "child-1");
    assert_eq!(state.codex_panel.outgoing.len(), 2);
    reply(&mut state, &second, "second client's child");
    assert!(state
        .codex_panel
        .activity_for("child-0")
        .expect("first")
        .reading());
    assert!(!state
        .codex_panel
        .activity_for("child-1")
        .expect("second")
        .reading());
    reply(&mut state, &first, "first client's child");
    assert_eq!(
        state
            .codex_panel
            .activity_for("child-0")
            .expect("first")
            .items()[0]["item"]["text"],
        "first client's child"
    );
    assert_eq!(
        state
            .codex_panel
            .activity_for("child-1")
            .expect("second")
            .items()[0]["item"]["text"],
        "second client's child"
    );
}

#[test]
fn releasing_one_interest_cancels_only_its_read_and_frees_a_bounded_slot() {
    let mut state = state();
    for index in 0..8 {
        read(&mut state, &format!("child-{index}"));
    }
    panel::details(
        &mut state,
        scope(Some(&PanelId::Agent("child-8".into()))),
        &mut Default::default(),
        false,
    );
    assert_eq!(state.codex_panel.activities.len(), 8);
    assert!(state
        .codex_panel
        .error()
        .expect("capacity error")
        .contains("eight"));
    let retired = state.codex_panel.activities["child-0"]
        .request
        .clone()
        .expect("old read");
    state.forget_codex_child("child-0");
    reply(&mut state, &retired, "late result");
    assert!(state.codex_panel.activity_for("child-0").is_none());
    read(&mut state, "child-8");
    assert_eq!(state.codex_panel.activities.len(), 8);
    assert!(state
        .codex_panel
        .activity_for("child-1")
        .expect("other client")
        .reading());
}

#[test]
fn several_child_views_share_one_byte_budget_and_a_full_view_does_not_erase_healthy_rows() {
    let mut state = state();
    let first = read(&mut state, "child-0");
    let second = read(&mut state, "child-1");
    let source = "x".repeat(5 * 1024 * 1024);
    reply(&mut state, &first, &source);
    reply(&mut state, &second, &source);
    let first = state
        .codex_panel
        .activity_for("child-0")
        .expect("healthy first");
    assert_eq!(
        first.items()[0]["item"]["text"]
            .as_str()
            .expect("source")
            .len(),
        source.len()
    );
    let second = state
        .codex_panel
        .activity_for("child-1")
        .expect("explicit failed second");
    assert!(second.error().expect("capacity failure").contains("8 MiB"));
    assert!(second.items().is_empty());
    let bytes: usize = state
        .codex_panel
        .activities
        .values()
        .map(|activity| activity.bytes + activity.tail.len())
        .sum();
    assert!(bytes <= MAX_BYTES);
}

#[test]
fn each_child_read_keeps_its_own_deadline_and_expiry_preserves_other_requests() {
    let mut state = state();
    read(&mut state, "child-0");
    let second = read(&mut state, "child-1");
    state
        .codex_panel
        .activities
        .get_mut("child-0")
        .expect("old read")
        .started = Instant::now() - Duration::from_secs(21);
    assert!(super::panel_reads::expire(&mut state));
    assert!(!state
        .codex_panel
        .activity_for("child-0")
        .expect("expired")
        .reading());
    assert!(state
        .codex_panel
        .activity_for("child-1")
        .expect("live")
        .reading());
    assert_eq!(state.codex_panel.outgoing.len(), 1);
    reply(&mut state, &second, "still served");
    assert!(!state
        .codex_panel
        .activity_for("child-1")
        .expect("completed")
        .reading());
}
