//! Keep native current-run reads tied to their task, root, connection and original deadline.

use std::time::{Duration, Instant};

use carlos_proto::ids::TaskId;
use serde_json::{json, Value};

use super::panel_events;
use super::panel_receive;
use super::panel_state::{PanelId, Request};
use crate::session::state::SessionState;

fn session() -> SessionState {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.enable_codex();
    session.set_codex_agents_for_test(
        (0..9)
            .map(|index| {
                json!({"id":format!("child-{index}"),
        "source":{"subAgent":{}},"parentThreadId":"root","status":{"type":"active"}})
            })
            .collect(),
    );
    session.verify_codex_agent_owners_for_test(session.codex_scope().epoch());
    session
}

fn start(session: &mut SessionState, index: usize) -> (TaskId, Request) {
    let task = session
        .task_page(None, None, 32)
        .expect("verified tasks")
        .tasks[index]
        .id
        .clone();
    session
        .prepare_task_run(&task, None, false)
        .expect("start exact native run read");
    let request = session.codex_panel.runs[&format!("child-{index}")]
        .request
        .clone()
        .expect("addressed read");
    (task, request)
}

fn reply(session: &mut SessionState, request: &Request, result: Value) {
    let selected = PanelId::Agent("child-8".into());
    let mut scope = session.backend_context().panel;
    scope.visible = false;
    scope.selected = Some(&selected);
    assert!(panel_receive::reply(
        session,
        scope,
        &mut Default::default(),
        &json!({
            "requestContext":{"operation":"panel","root":"root","epoch":0,"token":request.token},
            "requestMethod":request.method,"requestParams":request.params,"response":{"result":result}
        })
    ));
}

#[test]
fn clients_read_current_turns_out_of_order_without_using_the_selected_task() {
    let mut session = session();
    let before = session.task_page(None, None, 32).expect("unknown runs");
    let (first, first_request) = start(&mut session, 0);
    let (second, second_request) = start(&mut session, 1);
    assert_eq!(session.codex_panel.outgoing.len(), 2);
    assert_eq!(
        first_request.params,
        json!({"threadId":"child-0","sortDirection":"desc","itemsView":"notLoaded","limit":1})
    );
    reply(
        &mut session,
        &second_request,
        json!({"data":[{"id":"second-turn","status":"inProgress","items":[{"text":"not retained"}]}]}),
    );
    assert!(
        session
            .task_run_snapshot(&first)
            .expect("first read")
            .reading
    );
    let second_run = session.task_run_snapshot(&second).expect("second read");
    assert!(second_run.known);
    assert!(!second_run.reading);
    assert_eq!(second_run.native_state.as_deref(), Some("inProgress"));
    reply(
        &mut session,
        &first_request,
        json!({"data":[{"id":"first-turn","status":"completed"}]}),
    );
    let first_run = session.task_run_snapshot(&first).expect("first result");
    assert_eq!(first_run.native_state.as_deref(), Some("completed"));
    assert_ne!(first_run.run, second_run.run);
    let current = session.task_page(None, None, 32).expect("observed runs");
    assert_ne!(current.revision, before.revision);
    assert_eq!(current.tasks[0].run, first_run.run);
    assert!(session.codex_panel.activities.is_empty());
    assert_eq!(
        session.codex_panel.agents[0]["lastObservedTurn"],
        Value::Null
    );
    let encoded = serde_json::to_string(&second_run).expect("public run snapshot");
    assert!(!encoded.contains("second-turn"));
    assert!(!encoded.contains("child-1"));
}

#[test]
fn a_late_run_read_cannot_replace_a_newer_observed_turn_or_retarget_the_original_intent() {
    let mut session = session();
    let (task, pending) = start(&mut session, 0);
    let scope = session.backend_context().panel;
    assert!(panel_events::event(
        &mut session,
        scope,
        "turn/started",
        &json!({"threadId":"child-0",
        "turn":{"id":"replacement","status":"inProgress"}})
    ));
    let replacement = session
        .task_run_snapshot(&task)
        .expect("new observed turn")
        .run;
    reply(
        &mut session,
        &pending,
        json!({"data":[{"id":"retired","status":"inProgress"}]}),
    );
    let current = session
        .task_run_snapshot(&task)
        .expect("refused old intent");
    assert!(!current.reading);
    assert_eq!(current.run, replacement);
    assert!(current
        .error
        .expect("stale read diagnostic")
        .contains("not retargeted"));
    assert_eq!(
        session.codex_panel.agents[0]["lastObservedTurn"]["id"],
        "replacement"
    );
}

#[test]
fn run_reads_keep_separate_deadlines_and_never_evict_pending_targets_to_make_room() {
    let mut session = session();
    let mut reads = Vec::new();
    for index in 0..8 {
        reads.push(start(&mut session, index));
    }
    let ninth = session.task_page(None, None, 32).expect("tasks").tasks[8]
        .id
        .clone();
    assert!(session.prepare_task_run(&ninth, None, false).is_err());
    session
        .codex_panel
        .runs
        .get_mut("child-0")
        .expect("first")
        .started = Instant::now() - Duration::from_secs(21);
    assert!(super::panel_reads::expire(&mut session));
    assert!(session
        .task_run_snapshot(&reads[0].0)
        .expect("expired")
        .error
        .expect("timeout")
        .contains("20 seconds"));
    assert!(
        session
            .task_run_snapshot(&reads[1].0)
            .expect("healthy")
            .reading
    );
    assert_eq!(session.codex_panel.outgoing.len(), 7);
    session
        .prepare_task_run(&ninth, None, false)
        .expect("reuse a settled slot");
    assert_eq!(session.codex_panel.runs.len(), 8);
    assert!(!session.codex_panel.runs.contains_key("child-0"));
    reply(
        &mut session,
        &reads[0].1,
        json!({"data":[{"id":"late","status":"inProgress"}]}),
    );
    assert!(!session.codex_panel.runs.contains_key("child-0"));
    assert!(
        session
            .task_run_snapshot(&reads[1].0)
            .expect("other client unaffected")
            .reading
    );
}

#[test]
fn an_empty_read_stays_known_without_inventing_a_run_and_old_root_frames_are_ignored() {
    let mut session = session();
    let (empty, pending) = start(&mut session, 0);
    reply(&mut session, &pending, json!({"data":[]}));
    let empty = session
        .task_run_snapshot(&empty)
        .expect("empty native read");
    assert!(empty.known);
    assert!(!empty.reading);
    assert_eq!(empty.run, None);
    assert_eq!(empty.native_state, None);
    let (old, pending) = start(&mut session, 1);
    session.advance_codex_epoch();
    reply(
        &mut session,
        &pending,
        json!({"data":[{"id":"old","status":"inProgress"}]}),
    );
    assert!(session.task_run_snapshot(&old).is_err());
    assert!(session.codex_panel.runs["child-1"].turn.is_none());
}

#[test]
fn a_replacement_read_cannot_make_an_older_client_poll_follow_its_new_run() {
    let mut session = session();
    let (task, pending) = start(&mut session, 0);
    let first_read = session
        .task_run_snapshot(&task)
        .expect("first read")
        .read
        .expect("read id");
    reply(
        &mut session,
        &pending,
        json!({"data":[{"id":"first","status":"inProgress"}]}),
    );
    let first_run = session.task_run_snapshot(&task).expect("first result").run;
    session
        .prepare_task_run(&task, None, true)
        .expect("another client refreshes");
    let pending = session.codex_panel.runs["child-0"]
        .request
        .clone()
        .expect("replacement read");
    let second_read = session
        .task_run_snapshot(&task)
        .expect("replacement")
        .read
        .expect("new read id");
    assert_ne!(first_read, second_read);
    let refused = session
        .prepare_task_run(&task, Some(&first_read), false)
        .expect_err("old poll cannot follow replacement");
    assert!(refused.message.contains("not retargeted"));
    assert!(
        session
            .task_run_snapshot(&task)
            .expect("new read still pending")
            .reading
    );
    reply(
        &mut session,
        &pending,
        json!({"data":[{"id":"second","status":"inProgress"}]}),
    );
    assert_ne!(
        session.task_run_snapshot(&task).expect("new result").run,
        first_run
    );
    assert!(session
        .prepare_task_run(&task, Some(&first_read), false)
        .is_err());
    session
        .prepare_task_run(&task, Some(&second_read), false)
        .expect("original matching poll accepted");
}

#[test]
fn a_fresh_native_read_and_its_empty_result_override_older_observed_turn_metadata() {
    let mut session = session();
    session.codex_panel.agents[0]["lastObservedTurn"] =
        json!({"id":"observed-old","status":"completed"});
    let page = session
        .task_page(None, None, 32)
        .expect("previously observed turn");
    let task = &page.tasks[0].id;
    session
        .prepare_task_run(task, None, true)
        .expect("explicitly refresh current run");
    let pending = session.codex_panel.runs["child-0"]
        .request
        .clone()
        .expect("fresh native read");
    reply(
        &mut session,
        &pending,
        json!({"data":[{"id":"native-current","status":"inProgress"}]}),
    );
    let current = session
        .task_run_snapshot(task)
        .expect("current native result");
    assert_eq!(current.native_state.as_deref(), Some("inProgress"));
    assert_ne!(current.run, page.tasks[0].run);
    session
        .prepare_task_run(task, None, true)
        .expect("read current native state again");
    let pending = session.codex_panel.runs["child-0"]
        .request
        .clone()
        .expect("fresh empty read");
    reply(&mut session, &pending, json!({"data":[]}));
    let empty = session
        .task_run_snapshot(task)
        .expect("known empty native result");
    assert!(empty.known);
    assert_eq!(empty.run, None);
    assert_eq!(empty.native_state, None);
}

#[test]
fn run_reply_parameters_must_match_and_exhausted_tokens_cannot_reuse_a_read_identity() {
    let mut session = session();
    let (task, pending) = start(&mut session, 0);
    let mut wrong = pending.clone();
    wrong.params["threadId"] = json!("child-1");
    reply(
        &mut session,
        &wrong,
        json!({"data":[{"id":"foreign-turn","status":"inProgress"}]}),
    );
    assert!(
        session
            .task_run_snapshot(&task)
            .expect("original read still pending")
            .reading
    );
    assert!(session.codex_panel.runs["child-0"].turn.is_none());
    reply(
        &mut session,
        &pending,
        json!({"data":[{"id":"matching-turn","status":"inProgress"}]}),
    );
    let accepted = session
        .task_run_snapshot(&task)
        .expect("original read accepted");
    session.codex_meta.serial = u64::MAX - 1;
    let refused = session
        .prepare_task_run(&task, None, true)
        .expect_err("saturated token must stay unused");
    assert!(refused.message.contains("exhausted"));
    assert_eq!(
        session
            .task_run_snapshot(&task)
            .expect("accepted cache preserved")
            .read,
        accepted.read
    );
    assert_eq!(session.codex_meta.serial, u64::MAX - 1);
}
