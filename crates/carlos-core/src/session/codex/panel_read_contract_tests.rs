//! Preserve native panel bounds, ancestry and exact controls without terminal state.
use super::panel_contract_support::{
    answer, calls, contexts, details, receive, refresh, scope, session, stop, tick,
};
use super::panel_state::PanelId;
use crate::session::test_backend::TestBackend;
use serde_json::{json, Value};
fn agent(id: &str, parent: &str) -> Value {
    json!({"id":id,"parentThreadId":parent,"source":{"subAgent":{"thread_spawn":{
        "parent_thread_id":parent,"depth":2}}},"status":{"type":"notLoaded"}})
}

#[test]
fn an_unlisted_parent_is_verified_by_metadata_only_read_and_foreign_ancestry_is_excluded() {
    let mut session = session();
    let peer = TestBackend::default();
    refresh(&mut session, false);
    tick(&peer, &mut session);
    answer(
        &peer,
        &mut session,
        0,
        json!({"data":[agent("grand","parent")]}),
    );
    tick(&peer, &mut session);
    assert_eq!(
        calls(&peer)[1],
        (
            "thread/read".into(),
            json!({"threadId":"parent","includeTurns":false})
        )
    );
    answer(
        &peer,
        &mut session,
        1,
        json!({"thread":agent("parent","foreign")}),
    );
    tick(&peer, &mut session);
    answer(
        &peer,
        &mut session,
        2,
        json!({"thread":{"id":"foreign","source":"cli"}}),
    );
    tick(&peer, &mut session);
    answer(&peer, &mut session, 3, json!({"data":[]}));
    assert!(session.codex_panel().agents().is_empty());
    assert!(calls(&peer)
        .iter()
        .all(|(method, _)| method != "thread/resume"));
}

#[test]
fn more_than_256_historical_children_leave_a_usable_explicit_older_window() {
    let mut session = session();
    let peer = TestBackend::default();
    let agents: Vec<Value> = (0..256)
        .map(|index| agent(&format!("child-{index}"), "root"))
        .collect();
    refresh(&mut session, false);
    tick(&peer, &mut session);
    answer(
        &peer,
        &mut session,
        0,
        json!({"data":agents,"nextCursor":"older-window"}),
    );
    tick(&peer, &mut session);
    answer(&peer, &mut session, 1, json!({"data":[]}));
    assert_eq!(session.codex_panel().agents().len(), 256);
    assert_eq!(session.codex_panel().older(), "older-window");
    assert!(session.codex_panel().notice().contains("Bounded"));
    let selected = Some(PanelId::Agent("child-0".into()));
    refresh(&mut session, true);
    tick(&peer, &mut session);
    assert_eq!(calls(&peer)[2].1["cursor"], "older-window");
    answer(
        &peer,
        &mut session,
        2,
        json!({"data":[agent("older-child","root")]}),
    );
    tick(&peer, &mut session);
    answer(&peer, &mut session, 3, json!({"data":[]}));
    assert_eq!(session.codex_panel().agents().len(), 1);

    assert!(!session.codex_panel().contains_target(selected.as_ref()));
    assert!(session.codex_panel().older_window());
    assert!(session.codex_panel().notice().contains("Bounded"));
}

#[test]
fn repeated_cursors_or_oversized_snapshots_keep_previous_rows_and_healthy_root() {
    let mut session = session();
    let peer = TestBackend::default();
    session.push_codex_agent_for_test(agent("existing", "root"));

    refresh(&mut session, false);
    tick(&peer, &mut session);
    answer(
        &peer,
        &mut session,
        0,
        json!({"data":[agent("a","root")],"nextCursor":"loop"}),
    );
    tick(&peer, &mut session);
    answer(
        &peer,
        &mut session,
        1,
        json!({"data":[agent("b","root")],"nextCursor":"loop"}),
    );
    assert!(session.codex_panel().snapshot().is_none());
    assert!(session
        .codex_panel()
        .error()
        .expect("paging error")
        .contains("Repeated"));
    assert_eq!(session.codex_panel().agents()[0]["id"], "existing");

    refresh(&mut session, false);
    tick(&peer, &mut session);
    session.set_codex_panel_snapshot_bytes_for_test(8 * 1024 * 1024);
    answer(&peer, &mut session, 2, json!({"data":[]}));
    assert!(session
        .codex_panel()
        .error()
        .expect("bound error")
        .contains("8 MiB"));
    assert!(!session.codex_scope().closed());
}

#[test]
fn child_items_have_explicit_older_pages_and_live_output_is_bounded_without_main_history() {
    let mut session = session();
    let peer = TestBackend::default();
    session.push_codex_agent_for_test(agent("child", "root"));
    let selected = Some(PanelId::Agent("child".into()));
    details(&mut session, selected.as_ref(), false);
    for index in 0..20 {
        tick(&peer, &mut session);
        answer(
            &peer,
            &mut session,
            index,
            json!({"data":[{"turnId":"ct","item":{
            "id":format!("item-{index}"),"type":"agentMessage","text":"Child-only text"}}],
            "nextCursor":format!("older-{index}")}),
        );
    }
    let activity = session.codex_panel().activity().expect("child view");
    assert!(!activity.reading());
    assert!(activity.more());
    assert_eq!(activity.items().len(), 20);
    assert!(session.messages().is_empty());
    receive(
        &mut session,
        json!({"method":"item/commandExecution/outputDelta","params":{
        "threadId":"child","turnId":"ct","itemId":"live-command","delta":"日本語".repeat(10000)}}),
    );
    assert!(
        session
            .codex_panel()
            .activity()
            .expect("bounded tail")
            .tail()
            .len()
            <= 16 * 1024
    );
    assert!(session.messages().is_empty());
    details(&mut session, selected.as_ref(), true);
    tick(&peer, &mut session);
    assert_eq!(calls(&peer)[20].1["cursor"], "older-19");
}

#[test]
fn panel_and_detail_pages_keep_their_original_deadline_and_expired_work_is_not_sent() {
    let mut session = session();
    let peer = TestBackend::default();
    refresh(&mut session, false);
    session.set_codex_panel_snapshot_time_for_test(
        std::time::Instant::now() - std::time::Duration::from_millis(19900),
    );
    tick(&peer, &mut session);
    assert!(
        contexts(&peer)[0]["timeoutMs"]
            .as_u64()
            .expect("remaining deadline")
            <= 100
    );
    session.set_codex_panel_snapshot_time_for_test(
        std::time::Instant::now() - std::time::Duration::from_secs(21),
    );
    tick(&peer, &mut session);
    assert!(session.codex_panel().snapshot().is_none());
    session.push_codex_agent_for_test(agent("child", "root"));
    let selected = Some(PanelId::Agent("child".into()));
    details(&mut session, selected.as_ref(), false);
    session.set_codex_panel_activity_time_for_test(
        std::time::Instant::now() - std::time::Duration::from_millis(19900),
    );
    tick(&peer, &mut session);
    assert!(
        contexts(&peer)[1]["timeoutMs"]
            .as_u64()
            .expect("remaining deadline")
            <= 100
    );
    session.set_codex_panel_activity_time_for_test(
        std::time::Instant::now() - std::time::Duration::from_secs(21),
    );
    tick(&peer, &mut session);
    assert!(!session
        .codex_panel()
        .activity()
        .expect("activity")
        .reading());
    assert_eq!(calls(&peer).len(), 2);
    assert!(!session.codex_scope().closed());
}

#[test]
fn an_uncertain_selected_stop_retains_its_error_and_reconciles_with_reads_only() {
    let mut session = session();
    let peer = TestBackend::default();

    session.push_codex_process_for_test(crate::session::codex::panel_state::Process::new(
        "root".into(),
        json!({"processId":"55","command":"fixture"}),
    ));
    let selected = Some(PanelId::Process {
        item: None,
        thread: "root".into(),
        process: "55".into(),
    });
    stop(&mut session, selected.as_ref());
    tick(&peer, &mut session);
    let request = session
        .codex_panel()
        .stop()
        .as_ref()
        .expect("stop")
        .request()
        .clone();
    super::panel_receive::failed(
        &mut session,
        scope(None),
        &mut Default::default(),
        &request,
        "stop not confirmed; it may still complete",
    );
    tick(&peer, &mut session);
    assert_eq!(calls(&peer)[0].0, "thread/backgroundTerminals/terminate");
    assert_eq!(calls(&peer)[1].0, "thread/list");
    answer(&peer, &mut session, 1, json!({"data":[]}));
    tick(&peer, &mut session);
    answer(&peer, &mut session, 2, json!({"data":[]}));
    assert!(session
        .codex_panel()
        .control_error()
        .expect("retained stop uncertainty")
        .contains("not confirmed"));
    assert!(session.codex_panel().stop().is_none());
    assert!(
        calls(&peer)
            .iter()
            .filter(|(method, _)| method.ends_with("/terminate"))
            .count()
            == 1
    );
}

#[test]
fn successful_agent_stop_from_details_schedules_a_read_without_waiting_for_periodic_poll() {
    let mut session = session();
    let peer = TestBackend::default();

    session.push_codex_agent_for_test(agent("child", "root"));
    let selected = Some(PanelId::Agent("child".into()));
    stop(&mut session, selected.as_ref());
    tick(&peer, &mut session);
    answer(
        &peer,
        &mut session,
        0,
        json!({"data":[{"id":"active","status":"inProgress"}]}),
    );
    tick(&peer, &mut session);
    answer(&peer, &mut session, 1, json!({}));
    tick(&peer, &mut session);
    assert_eq!(calls(&peer)[2].0, "thread/list");

    assert!(session.codex_panel().stop().is_none());
}
