//! Bound panel windows and verify ancestry without hydrating or resuming a child.

use serde_json::{json, Value};

use super::codex_question_support::{app, receive, Peer};
use crate::app::codex_panel::{details, refresh, tick};
use crate::app::codex_panel_state::PanelId;

fn agent(id: &str, parent: &str) -> Value {
    json!({"id":id,"parentThreadId":parent,"source":{"subAgent":{"thread_spawn":{
        "parent_thread_id":parent,"depth":2}}},"status":{"type":"notLoaded"}})
}

fn answer(peer: &Peer, app: &mut crate::app::AppState, index: usize, result: Value) {
    let (method, params) = peer.controls.borrow()[index].clone();
    receive(
        app,
        json!({"method":"carlos/codex/controlReply","params":{
        "requestMethod":method,"requestParams":params,"requestContext":peer.contexts.borrow()[index],
        "response":{"result":result}}}),
    );
}

#[test]
fn an_unlisted_parent_is_verified_by_metadata_only_read_and_foreign_ancestry_is_excluded() {
    let mut app = app();
    let peer = Peer::default();
    refresh(&mut app, false);
    tick(&peer, &mut app);
    answer(
        &peer,
        &mut app,
        0,
        json!({"data":[agent("grand","parent")]}),
    );
    tick(&peer, &mut app);
    assert_eq!(
        peer.controls.borrow()[1],
        (
            "thread/read".into(),
            json!({"threadId":"parent","includeTurns":false})
        )
    );
    answer(
        &peer,
        &mut app,
        1,
        json!({"thread":agent("parent","foreign")}),
    );
    tick(&peer, &mut app);
    answer(
        &peer,
        &mut app,
        2,
        json!({"thread":{"id":"foreign","source":"cli"}}),
    );
    tick(&peer, &mut app);
    answer(&peer, &mut app, 3, json!({"data":[]}));
    assert!(app.codex.panel.agents.is_empty());
    assert!(peer
        .controls
        .borrow()
        .iter()
        .all(|(method, _)| method != "thread/resume"));
}

#[test]
fn more_than_256_historical_children_leave_a_usable_explicit_older_window() {
    let mut app = app();
    let peer = Peer::default();
    let agents: Vec<Value> = (0..256)
        .map(|index| agent(&format!("child-{index}"), "root"))
        .collect();
    refresh(&mut app, false);
    tick(&peer, &mut app);
    answer(
        &peer,
        &mut app,
        0,
        json!({"data":agents,"nextCursor":"older-window"}),
    );
    tick(&peer, &mut app);
    answer(&peer, &mut app, 1, json!({"data":[]}));
    assert_eq!(app.codex.panel.agents.len(), 256);
    assert_eq!(app.codex.panel.older, "older-window");
    assert!(app.codex.panel.notice.contains("Bounded"));
    let selected = app.codex.panel.selected.clone();
    refresh(&mut app, true);
    tick(&peer, &mut app);
    assert_eq!(peer.controls.borrow()[2].1["cursor"], "older-window");
    answer(
        &peer,
        &mut app,
        2,
        json!({"data":[agent("older-child","root")]}),
    );
    tick(&peer, &mut app);
    answer(&peer, &mut app, 3, json!({"data":[]}));
    assert_eq!(app.codex.panel.agents.len(), 1);
    assert_eq!(app.codex.panel.selected, selected);
    assert!(!app.codex.panel.selected_exists());
    assert!(app.codex.panel.older_window);
    assert!(app.codex.panel.notice.contains("Bounded"));
}

#[test]
fn repeated_cursors_or_oversized_snapshots_keep_previous_rows_and_main_draft() {
    let mut app = app();
    let peer = Peer::default();
    app.codex.panel.agents.push(agent("existing", "root"));
    app.set_input_text("preserve 日本語 draft");
    refresh(&mut app, false);
    tick(&peer, &mut app);
    answer(
        &peer,
        &mut app,
        0,
        json!({"data":[agent("a","root")],"nextCursor":"loop"}),
    );
    tick(&peer, &mut app);
    answer(
        &peer,
        &mut app,
        1,
        json!({"data":[agent("b","root")],"nextCursor":"loop"}),
    );
    assert!(app.codex.panel.snapshot.is_none());
    assert!(app
        .codex
        .panel
        .error
        .as_deref()
        .expect("paging error")
        .contains("Repeated"));
    assert_eq!(app.codex.panel.agents[0]["id"], "existing");
    assert_eq!(app.input_text(), "preserve 日本語 draft");
    refresh(&mut app, false);
    tick(&peer, &mut app);
    app.codex.panel.snapshot.as_mut().expect("read").bytes = 8 * 1024 * 1024;
    answer(&peer, &mut app, 2, json!({"data":[]}));
    assert!(app
        .codex
        .panel
        .error
        .as_deref()
        .expect("bound error")
        .contains("8 MiB"));
    assert!(!app.codex.closed);
}

#[test]
fn child_items_have_explicit_older_pages_and_live_output_is_bounded_without_main_history() {
    let mut app = app();
    let peer = Peer::default();
    app.codex.panel.agents.push(agent("child", "root"));
    app.codex.panel.selected = Some(PanelId::Agent("child".into()));
    details(&mut app, false);
    for index in 0..20 {
        tick(&peer, &mut app);
        answer(
            &peer,
            &mut app,
            index,
            json!({"data":[{"turnId":"ct","item":{
            "id":format!("item-{index}"),"type":"agentMessage","text":"Child-only text"}}],
            "nextCursor":format!("older-{index}")}),
        );
    }
    let activity = app.codex.panel.activity.as_ref().expect("child view");
    assert!(activity.request.is_none());
    assert!(activity.more);
    assert_eq!(activity.items.len(), 20);
    assert!(app.messages.is_empty());
    receive(
        &mut app,
        json!({"method":"item/commandExecution/outputDelta","params":{
        "threadId":"child","turnId":"ct","itemId":"live-command","delta":"日本語".repeat(10000)}}),
    );
    assert!(
        app.codex
            .panel
            .activity
            .as_ref()
            .expect("bounded tail")
            .tail
            .len()
            <= 16 * 1024
    );
    assert!(app.messages.is_empty());
    details(&mut app, true);
    tick(&peer, &mut app);
    assert_eq!(peer.controls.borrow()[20].1["cursor"], "older-19");
}

#[test]
fn panel_and_detail_pages_keep_their_original_deadline_and_expired_work_is_not_sent() {
    let mut app = app();
    let peer = Peer::default();
    refresh(&mut app, false);
    app.codex.panel.snapshot.as_mut().expect("snapshot").started =
        std::time::Instant::now() - std::time::Duration::from_millis(19900);
    tick(&peer, &mut app);
    assert!(
        peer.contexts.borrow()[0]["timeoutMs"]
            .as_u64()
            .expect("remaining deadline")
            <= 100
    );
    app.codex.panel.snapshot.as_mut().expect("snapshot").started =
        std::time::Instant::now() - std::time::Duration::from_secs(21);
    tick(&peer, &mut app);
    assert!(app.codex.panel.snapshot.is_none());
    app.codex.panel.agents.push(agent("child", "root"));
    app.codex.panel.selected = Some(PanelId::Agent("child".into()));
    details(&mut app, false);
    app.codex.panel.activity.as_mut().expect("activity").started =
        std::time::Instant::now() - std::time::Duration::from_millis(19900);
    tick(&peer, &mut app);
    assert!(
        peer.contexts.borrow()[1]["timeoutMs"]
            .as_u64()
            .expect("remaining deadline")
            <= 100
    );
    app.codex.panel.activity.as_mut().expect("activity").started =
        std::time::Instant::now() - std::time::Duration::from_secs(21);
    tick(&peer, &mut app);
    assert!(app
        .codex
        .panel
        .activity
        .as_ref()
        .expect("activity")
        .request
        .is_none());
    assert_eq!(peer.controls.borrow().len(), 2);
    assert!(!app.codex.closed);
}

#[test]
fn an_uncertain_selected_stop_retains_its_error_and_reconciles_with_reads_only() {
    let mut app = app();
    let peer = Peer::default();
    app.codex.panel.show = true;
    app.codex.panel.detail = true;
    app.codex
        .panel
        .processes
        .push(crate::app::codex_panel_state::Process {
            thread: "root".into(),
            data: json!({"processId":"55","command":"fixture"}),
        });
    app.codex.panel.selected = Some(PanelId::Process {
        item: None,
        thread: "root".into(),
        process: "55".into(),
    });
    crate::app::codex_panel::stop(&mut app);
    tick(&peer, &mut app);
    let request = app.codex.panel.stop.as_ref().expect("stop").request.clone();
    crate::app::codex_panel_receive::failed(
        &mut app,
        &request,
        "stop not confirmed; it may still complete",
    );
    tick(&peer, &mut app);
    assert_eq!(
        peer.controls.borrow()[0].0,
        "thread/backgroundTerminals/terminate"
    );
    assert_eq!(peer.controls.borrow()[1].0, "thread/list");
    answer(&peer, &mut app, 1, json!({"data":[]}));
    tick(&peer, &mut app);
    answer(&peer, &mut app, 2, json!({"data":[]}));
    assert!(app
        .codex
        .panel
        .control_error
        .as_deref()
        .expect("retained stop uncertainty")
        .contains("not confirmed"));
    assert!(app.codex.panel.stop.is_none());
    assert!(
        peer.controls
            .borrow()
            .iter()
            .filter(|(method, _)| method.ends_with("/terminate"))
            .count()
            == 1
    );
}

#[test]
fn successful_agent_stop_from_details_schedules_a_read_without_waiting_for_periodic_poll() {
    let mut app = app();
    let peer = Peer::default();
    app.codex.panel.show = true;
    app.codex.panel.detail = true;
    app.codex.panel.agents.push(agent("child", "root"));
    app.codex.panel.selected = Some(PanelId::Agent("child".into()));
    crate::app::codex_panel::stop(&mut app);
    tick(&peer, &mut app);
    answer(
        &peer,
        &mut app,
        0,
        json!({"data":[{"id":"active","status":"inProgress"}]}),
    );
    tick(&peer, &mut app);
    answer(&peer, &mut app, 1, json!({}));
    tick(&peer, &mut app);
    assert_eq!(peer.controls.borrow()[2].0, "thread/list");
    assert!(app.codex.panel.detail);
    assert!(app.codex.panel.stop.is_none());
}
