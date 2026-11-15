//! Preserve native panel bounds, ancestry and exact controls without terminal state.
use super::panel_contract_support::{
    answer, calls, details, receive, session, snapshot, stop, tick,
};
use super::panel_state::PanelId;
use crate::session::test_backend::TestBackend;
use serde_json::{json, Value};
fn agent(id: &str, parent: &str) -> Value {
    json!({"id":id,"parentThreadId":parent,"agentNickname":id,"agentRole":"worker",
        "status":{"type":"active","activeFlags":[]},"canAcceptDirectInput":false,
        "source":{"subAgent":{"thread_spawn":{"parent_thread_id":parent,"depth":1}}}})
}

#[test]
fn verified_descendants_exclude_unrelated_chats_even_if_native_ignores_the_filter() {
    let mut session = session();
    let peer = TestBackend::default();
    let unrelated = json!({"id":"unrelated","source":"cli","status":{"type":"idle"}});
    snapshot(
        &peer,
        &mut session,
        json!([agent("grand", "child"), unrelated, agent("child", "root")]),
    );
    assert_eq!(session.codex_panel().agents().len(), 2);
    assert!(calls(&peer)
        .iter()
        .all(|(_, params)| params["threadId"] != "unrelated"));
    assert_eq!(calls(&peer)[0].1["ancestorThreadId"], "root");
    assert!(
        calls(&peer)[0].1["sourceKinds"]
            .as_array()
            .expect("sources")
            .len()
            > 1
    );
    let selected = Some(PanelId::Agent("grand".into()));
    stop(&mut session, selected.as_ref());
    tick(&peer, &mut session);
    let index = calls(&peer).len() - 1;
    assert_eq!(calls(&peer)[index].1["threadId"], "grand");
    answer(
        &peer,
        &mut session,
        index,
        json!({"data":[{"id":"grand-current","status":"inProgress"}]}),
    );
    tick(&peer, &mut session);
    let index = calls(&peer).len() - 1;
    assert_eq!(
        calls(&peer)[index],
        (
            "turn/interrupt".into(),
            json!({"threadId":"grand","turnId":"grand-current"})
        )
    );
    assert!(session.active_turn_id().is_none());
    assert_eq!(session.thread_id(), "root");
}

#[test]
fn child_text_tools_and_embedded_reports_stay_out_of_main_live_and_resumed_history() {
    let mut session = session();
    let peer = TestBackend::default();
    snapshot(&peer, &mut session, json!([agent("child", "root")]));
    let selected = Some(PanelId::Agent("child".into()));
    details(&mut session, selected.as_ref(), false);
    for (id, kind, text) in [
        ("child-text", "agentMessage", "CHILD_BODY"),
        ("child-tool", "commandExecution", "CHILD_TOOL"),
    ] {
        receive(
            &mut session,
            json!({"method":"item/completed","params":{"threadId":"child",
            "turnId":"ct","item":{"id":id,"type":kind,"text":text,"command":text}}}),
        );
    }
    let call = json!({"id":"root-wait","type":"collabAgentToolCall","tool":"wait",
        "status":"completed","senderThreadId":"root","receiverThreadIds":["child"],
        "agentsStates":{"child":{"status":"completed","message":"ROOT_EMBEDDED_CHILD_REPORT"}}});
    receive(
        &mut session,
        json!({"method":"item/completed","params":{"threadId":"root","turnId":"rt","item":call}}),
    );
    assert!(session
        .messages()
        .iter()
        .all(|row| !row.text.contains("CHILD")));
    assert_eq!(
        session.codex_panel().reports()["child"]["message"],
        "ROOT_EMBEDDED_CHILD_REPORT"
    );
    assert!(
        session
            .codex_panel()
            .activity()
            .expect("details")
            .items()
            .len()
            >= 2
    );
    let response = json!({"result":{"thread":{"id":"root","turns":[{"id":"rt",
        "status":"completed","items":[call]}]}}});
    crate::session::item_history::load_history_from_start_or_resume(
        &mut session,
        &response.to_string(),
    )
    .expect("resume");
    assert!(session
        .messages()
        .iter()
        .all(|row| !row.text.contains("ROOT_EMBEDDED_CHILD_REPORT")));
    assert_eq!(
        session.codex_panel().reports()["child"]["message"],
        "ROOT_EMBEDDED_CHILD_REPORT"
    );
}

#[test]
fn vanished_selection_does_not_retarget_stop_and_old_generation_results_cannot_reopen_panel() {
    let mut session = session();
    let peer = TestBackend::default();
    snapshot(
        &peer,
        &mut session,
        json!([agent("a", "root"), agent("b", "root")]),
    );
    let mut selected = Some(PanelId::Agent("a".into()));
    snapshot(&peer, &mut session, json!([agent("b", "root")]));
    let before = calls(&peer).len();
    stop(&mut session, selected.as_ref());
    assert!(!tick(&peer, &mut session));
    assert_eq!(calls(&peer).len(), before);
    assert!(session
        .codex_panel()
        .error()
        .expect("visible failure")
        .contains("left"));
    selected = Some(PanelId::Agent("b".into()));
    stop(&mut session, selected.as_ref());
    tick(&peer, &mut session);
    let index = calls(&peer).len() - 1;
    session.adopt_native_thread("new-root".into());
    answer(
        &peer,
        &mut session,
        index,
        json!({"data":[{"id":"late","status":"inProgress"}]}),
    );
    assert!(!session.codex_panel().has_outgoing());
    assert!(session.codex_panel().agents().is_empty());
}

#[test]
fn idle_agents_are_not_completed_and_process_stop_uses_both_native_ids() {
    let mut session = session();
    let peer = TestBackend::default();
    snapshot(&peer, &mut session, json!([agent("child", "root")]));
    let mut selected = Some(PanelId::Agent("child".into()));
    receive(
        &mut session,
        json!({"method":"thread/status/changed","params":{
        "threadId":"child","status":{"type":"idle"}}}),
    );
    assert_eq!(session.codex_panel().agents()[0]["status"]["type"], "idle");
    stop(&mut session, selected.as_ref());
    tick(&peer, &mut session);
    let index = calls(&peer).len() - 1;
    answer(
        &peer,
        &mut session,
        index,
        json!({"data":[{"id":"done","status":"completed"}]}),
    );
    assert!(session.codex_panel().stop().is_none());
    session.push_codex_process_for_test(crate::session::codex::panel_state::Process::new(
        "child".into(),
        json!({"processId":"88","itemId":"exec","command":"sleep60","cwd":"/tmp"}),
    ));
    selected = Some(PanelId::Process {
        item: Some("exec".into()),
        thread: "child".into(),
        process: "88".into(),
    });
    stop(&mut session, selected.as_ref());
    tick(&peer, &mut session);
    assert_eq!(
        calls(&peer).last().expect("terminate"),
        &(
            "thread/backgroundTerminals/terminate".into(),
            json!({"threadId":"child","processId":"88"})
        )
    );
    assert!(calls(&peer)
        .iter()
        .all(|(method, _)| !method.ends_with("/clean")));
}
