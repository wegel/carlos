//! Bound panel windows and verify ancestry without hydrating or resuming a child.

use serde_json::{json, Value};

use super::codex_question_support::{app, receive, Peer};

use crate::app::codex_panel::{details, refresh, tick};

use carlos_core::session::codex::panel_state::PanelId;

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
    assert!(app.session.codex_panel().agents().is_empty());
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
    assert_eq!(app.session.codex_panel().agents().len(), 256);
    assert_eq!(app.session.codex_panel().older(), "older-window");
    assert!(app.session.codex_panel().notice().contains("Bounded"));
    let selected = app.view.codex_panel.selected.clone();
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
    assert_eq!(app.session.codex_panel().agents().len(), 1);
    assert_eq!(app.view.codex_panel.selected, selected);
    assert!(!app
        .session
        .codex_panel()
        .contains_target(app.view.codex_panel.selected.as_ref()));
    assert!(app.session.codex_panel().older_window());
    assert!(app.session.codex_panel().notice().contains("Bounded"));
}

#[test]
fn repeated_cursors_or_oversized_snapshots_keep_previous_rows_and_main_draft() {
    let mut app = app();
    let peer = Peer::default();
    app.session
        .push_codex_agent_for_test(agent("existing", "root"));
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
    assert!(app.session.codex_panel().snapshot().is_none());
    assert!(app
        .session
        .codex_panel()
        .error()
        .expect("paging error")
        .contains("Repeated"));
    assert_eq!(app.session.codex_panel().agents()[0]["id"], "existing");
    assert_eq!(app.input_text(), "preserve 日本語 draft");
    refresh(&mut app, false);
    tick(&peer, &mut app);
    app.session
        .set_codex_panel_snapshot_bytes_for_test(8 * 1024 * 1024);
    answer(&peer, &mut app, 2, json!({"data":[]}));
    assert!(app
        .session
        .codex_panel()
        .error()
        .expect("bound error")
        .contains("8 MiB"));
    assert!(!app.session.codex_scope().closed());
}

#[test]
fn child_items_have_explicit_older_pages_and_live_output_is_bounded_without_main_history() {
    let mut app = app();
    let peer = Peer::default();
    app.session
        .push_codex_agent_for_test(agent("child", "root"));
    app.view.codex_panel.selected = Some(PanelId::Agent("child".into()));
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
    let activity = app.session.codex_panel().activity().expect("child view");
    assert!(!activity.reading());
    assert!(activity.more());
    assert_eq!(activity.items().len(), 20);
    assert!(app.session.messages().is_empty());
    receive(
        &mut app,
        json!({"method":"item/commandExecution/outputDelta","params":{
        "threadId":"child","turnId":"ct","itemId":"live-command","delta":"日本語".repeat(10000)}}),
    );
    assert!(
        app.session
            .codex_panel()
            .activity()
            .expect("bounded tail")
            .tail()
            .len()
            <= 16 * 1024
    );
    assert!(app.session.messages().is_empty());
    details(&mut app, true);
    tick(&peer, &mut app);
    assert_eq!(peer.controls.borrow()[20].1["cursor"], "older-19");
}

#[test]
fn panel_and_detail_pages_keep_their_original_deadline_and_expired_work_is_not_sent() {
    let mut app = app();
    let peer = Peer::default();
    refresh(&mut app, false);
    app.session.set_codex_panel_snapshot_time_for_test(
        std::time::Instant::now() - std::time::Duration::from_millis(19900),
    );
    tick(&peer, &mut app);
    assert!(
        peer.contexts.borrow()[0]["timeoutMs"]
            .as_u64()
            .expect("remaining deadline")
            <= 100
    );
    app.session.set_codex_panel_snapshot_time_for_test(
        std::time::Instant::now() - std::time::Duration::from_secs(21),
    );
    tick(&peer, &mut app);
    assert!(app.session.codex_panel().snapshot().is_none());
    app.session
        .push_codex_agent_for_test(agent("child", "root"));
    app.view.codex_panel.selected = Some(PanelId::Agent("child".into()));
    details(&mut app, false);
    app.session.set_codex_panel_activity_time_for_test(
        std::time::Instant::now() - std::time::Duration::from_millis(19900),
    );
    tick(&peer, &mut app);
    assert!(
        peer.contexts.borrow()[1]["timeoutMs"]
            .as_u64()
            .expect("remaining deadline")
            <= 100
    );
    app.session.set_codex_panel_activity_time_for_test(
        std::time::Instant::now() - std::time::Duration::from_secs(21),
    );
    tick(&peer, &mut app);
    assert!(!app
        .session
        .codex_panel()
        .activity()
        .expect("activity")
        .reading());
    assert_eq!(peer.controls.borrow().len(), 2);
    assert!(!app.session.codex_scope().closed());
}

#[test]
fn an_uncertain_selected_stop_retains_its_error_and_reconciles_with_reads_only() {
    let mut app = app();
    let peer = Peer::default();
    app.view.codex_panel.show = true;
    app.view.codex_panel.detail = true;
    app.session.push_codex_process_for_test(
        carlos_core::session::codex::panel_state::Process::new(
            "root".into(),
            json!({"processId":"55","command":"fixture"}),
        ),
    );
    app.view.codex_panel.selected = Some(PanelId::Process {
        item: None,
        thread: "root".into(),
        process: "55".into(),
    });
    crate::app::codex_panel::stop(&mut app);
    tick(&peer, &mut app);
    let request = app
        .session
        .codex_panel()
        .stop()
        .as_ref()
        .expect("stop")
        .request()
        .clone();
    crate::app::tests::codex_panel_support::failed(
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
        .session
        .codex_panel()
        .control_error()
        .expect("retained stop uncertainty")
        .contains("not confirmed"));
    assert!(app.session.codex_panel().stop().is_none());
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
    app.view.codex_panel.show = true;
    app.view.codex_panel.detail = true;
    app.session
        .push_codex_agent_for_test(agent("child", "root"));
    app.view.codex_panel.selected = Some(PanelId::Agent("child".into()));
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
    assert!(app.view.codex_panel.detail);
    assert!(app.session.codex_panel().stop().is_none());
}
