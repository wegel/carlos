//! Keep native goal values replayable, revision-checked and independent of private editor failures.

use carlos_proto::events::Event;
use carlos_proto::goals::GoalKnowledge;
use serde_json::{json, Value};

use super::codex::goal_receive;
use super::codex::goals;
use super::event_tests::synchronize;
use super::state::SessionState;
use super::test_backend::TestBackend;

fn session() -> SessionState {
    let mut core = SessionState::new("root".into(), Default::default(), false);
    core.enable_codex();
    core
}

fn goal(objective: &str) -> Value {
    json!({"threadId":"root", "objective":objective, "status":"paused", "tokenBudget":null,
        "tokensUsed":9007199254740993u64, "timeUsedSeconds":17, "createdAt":1700000000,"updatedAt":1700000001})
}

fn observe(core: &mut SessionState, goal: Value) {
    goal_receive::observed(
        core,
        "thread/goal/updated",
        &json!({"threadId":"root", "goal":goal}),
    );
}

fn read_current(core: &mut SessionState) {
    let peer = TestBackend::default();
    let scope = core.codex_scope();
    core.refresh_codex_goal();
    assert!(goals::tick(
        &peer,
        core,
        scope,
        std::time::Instant::now(),
        false
    ));
    let call = peer.calls.lock().expect("native read")[0].clone();
    let goal = core.codex_goals.goal.clone();
    goal_receive::reply(
        core,
        scope,
        &mut Default::default(),
        &json!({
            "requestMethod":call.0, "requestParams":call.1, "requestContext":call.2,
            "response":{"result":{"goal":goal}}
        }),
    );
}

#[test]
fn snapshots_and_numbered_events_keep_goals_without_native_routes_or_private_mutation_errors() {
    let mut core = session();
    let mut client = core.publish().snapshot(100);
    assert_eq!(
        client.goal.as_ref().expect("Codex goal facts").knowledge,
        GoalKnowledge::Unknown
    );
    observe(&mut core, goal("Shared goal 🌍"));
    let events = synchronize(&mut core, &mut client);
    assert!(events
        .iter()
        .any(|event| matches!(event.event, Event::GoalChanged { .. })));
    let facts = client.goal.as_ref().expect("shared facts");
    assert_eq!(facts.knowledge, GoalKnowledge::Known);
    assert_eq!(
        facts.goal.as_ref().expect("value").tokens_used.get(),
        9007199254740993
    );
    assert!(!serde_json::to_string(facts)
        .expect("wire goal")
        .contains("threadId"));
    core.codex_goals.error = Some("PRIVATE MUTATION ERROR".into());
    assert!(core.publish().records().is_empty());
    assert!(!serde_json::to_string(&core.publish().snapshot(100))
        .expect("snapshot")
        .contains("PRIVATE MUTATION ERROR"));
    let previous = core.goal_revision();
    core.codex_goals.read_error = Some("shared read failure".into());
    synchronize(&mut core, &mut client);
    assert_eq!(
        client.goal.as_ref().expect("facts").read_error.as_deref(),
        Some("shared read failure")
    );
    assert_eq!(
        core.goal_revision(),
        previous,
        "a failed read does not change accepted goal values"
    );
}

#[test]
fn equal_native_reads_do_not_invalidate_an_editor_but_changed_values_roots_and_epochs_do() {
    let mut core = session();
    observe(&mut core, goal("original"));
    let revision = core.goal_revision();
    core.publish();
    read_current(&mut core);
    assert_eq!(core.goal_revision(), revision);
    assert!(core.publish().records().is_empty());
    observe(&mut core, goal("changed"));
    assert_ne!(core.goal_revision(), revision);
    let changed = core.goal_revision();
    core.advance_codex_epoch();
    assert_ne!(core.goal_revision(), changed);
    let epoch = core.goal_revision();
    core.adopt_native_thread("new-root".into());
    assert_ne!(core.goal_revision(), epoch);
}

#[test]
fn exhausted_goal_revisions_become_unknown_instead_of_reusing_an_old_write_precondition() {
    let mut core = session();
    observe(&mut core, goal("old"));
    core.codex_goals.revision = u64::MAX;
    let previous = core.goal_revision();
    observe(&mut core, goal("new"));
    let facts = core.publish().snapshot(100).goal.expect("goal facts");
    assert_eq!(facts.knowledge, GoalKnowledge::Unknown);
    assert_ne!(facts.revision, previous);
    assert_eq!(facts.goal.expect("last accepted value").objective, "old");
}
