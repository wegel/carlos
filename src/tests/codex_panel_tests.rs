//! Keep descendant/process controls exact and child details separate from the root transcript.

use serde_json::{json, Value};

use super::codex_question_support::{app, receive, Peer};
use crate::app::codex_panel::{details, refresh, stop, tick};
use crate::app::codex_panel_state::PanelId;
use crate::app::item_history::load_history_from_start_or_resume;

fn agent(id: &str, parent: &str) -> Value {
    json!({"id":id,"parentThreadId":parent,"agentNickname":id,"agentRole":"worker",
        "status":{"type":"active","activeFlags":[]},"canAcceptDirectInput":false,
        "source":{"subAgent":{"thread_spawn":{"parent_thread_id":parent,"depth":1}}}})
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

fn snapshot(peer: &Peer, app: &mut crate::app::AppState, agents: Value) {
    let mut next = peer.controls.borrow().len();
    refresh(app, false);
    for _ in 0..20 {
        if app.codex.panel.outgoing.is_empty() {
            break;
        }
        tick(peer, app);
        while next < peer.controls.borrow().len() {
            let method = peer.controls.borrow()[next].0.clone();
            let data = if method == "thread/list" {
                agents.clone()
            } else {
                json!([])
            };
            answer(peer, app, next, json!({"data":data,"nextCursor":null}));
            next += 1;
        }
    }
    assert!(app.codex.panel.snapshot.is_none());
}

#[test]
fn verified_descendants_exclude_unrelated_chats_even_if_native_ignores_the_filter() {
    let mut app = app();
    let peer = Peer::default();
    let unrelated = json!({"id":"unrelated","source":"cli","status":{"type":"idle"}});
    snapshot(
        &peer,
        &mut app,
        json!([agent("grand", "child"), unrelated, agent("child", "root")]),
    );
    assert_eq!(app.codex.panel.agents.len(), 2);
    assert!(peer
        .controls
        .borrow()
        .iter()
        .all(|(_, params)| params["threadId"] != "unrelated"));
    assert_eq!(peer.controls.borrow()[0].1["ancestorThreadId"], "root");
    assert!(
        peer.controls.borrow()[0].1["sourceKinds"]
            .as_array()
            .expect("sources")
            .len()
            > 1
    );
    app.codex.panel.selected = Some(PanelId::Agent("grand".into()));
    stop(&mut app);
    tick(&peer, &mut app);
    let index = peer.controls.borrow().len() - 1;
    assert_eq!(peer.controls.borrow()[index].1["threadId"], "grand");
    answer(
        &peer,
        &mut app,
        index,
        json!({"data":[{"id":"grand-current","status":"inProgress"}]}),
    );
    tick(&peer, &mut app);
    let index = peer.controls.borrow().len() - 1;
    assert_eq!(
        peer.controls.borrow()[index],
        (
            "turn/interrupt".into(),
            json!({"threadId":"grand","turnId":"grand-current"})
        )
    );
    assert!(app.active_turn_id.is_none());
    assert_eq!(app.thread_id, "root");
}

#[test]
fn child_text_tools_and_embedded_reports_stay_out_of_main_live_and_resumed_history() {
    let mut app = app();
    let peer = Peer::default();
    snapshot(&peer, &mut app, json!([agent("child", "root")]));
    app.codex.panel.selected = Some(PanelId::Agent("child".into()));
    details(&mut app, false);
    for (id, kind, text) in [
        ("child-text", "agentMessage", "CHILD_BODY"),
        ("child-tool", "commandExecution", "CHILD_TOOL"),
    ] {
        receive(
            &mut app,
            json!({"method":"item/completed","params":{"threadId":"child",
            "turnId":"ct","item":{"id":id,"type":kind,"text":text,"command":text}}}),
        );
    }
    let call = json!({"id":"root-wait","type":"collabAgentToolCall","tool":"wait",
        "status":"completed","senderThreadId":"root","receiverThreadIds":["child"],
        "agentsStates":{"child":{"status":"completed","message":"ROOT_EMBEDDED_CHILD_REPORT"}}});
    receive(
        &mut app,
        json!({"method":"item/completed","params":{"threadId":"root","turnId":"rt","item":call}}),
    );
    assert!(app.messages.iter().all(|row| !row.text.contains("CHILD")));
    assert_eq!(
        app.codex.panel.reports["child"]["message"],
        "ROOT_EMBEDDED_CHILD_REPORT"
    );
    assert!(
        app.codex
            .panel
            .activity
            .as_ref()
            .expect("details")
            .items
            .len()
            >= 2
    );
    let response = json!({"result":{"thread":{"id":"root","turns":[{"id":"rt",
        "status":"completed","items":[call]}]}}});
    load_history_from_start_or_resume(&mut app, &response.to_string()).expect("resume");
    assert!(app
        .messages
        .iter()
        .all(|row| !row.text.contains("ROOT_EMBEDDED_CHILD_REPORT")));
    assert_eq!(
        app.codex.panel.reports["child"]["message"],
        "ROOT_EMBEDDED_CHILD_REPORT"
    );
}

#[test]
fn vanished_selection_does_not_retarget_stop_and_old_generation_results_cannot_reopen_panel() {
    let mut app = app();
    let peer = Peer::default();
    snapshot(
        &peer,
        &mut app,
        json!([agent("a", "root"), agent("b", "root")]),
    );
    assert_eq!(app.codex.panel.selected, Some(PanelId::Agent("a".into())));
    snapshot(&peer, &mut app, json!([agent("b", "root")]));
    let before = peer.controls.borrow().len();
    stop(&mut app);
    assert!(!tick(&peer, &mut app));
    assert_eq!(peer.controls.borrow().len(), before);
    assert!(app
        .codex
        .panel
        .error
        .as_deref()
        .expect("visible failure")
        .contains("left"));
    app.codex.panel.select(true);
    stop(&mut app);
    tick(&peer, &mut app);
    let index = peer.controls.borrow().len() - 1;
    app.set_thread_id("new-root");
    answer(
        &peer,
        &mut app,
        index,
        json!({"data":[{"id":"late","status":"inProgress"}]}),
    );
    assert!(app.codex.panel.outgoing.is_empty());
    assert!(app.codex.panel.agents.is_empty());
}

#[test]
fn idle_agents_are_not_completed_and_process_stop_uses_both_native_ids() {
    let mut app = app();
    let peer = Peer::default();
    snapshot(&peer, &mut app, json!([agent("child", "root")]));
    receive(
        &mut app,
        json!({"method":"thread/status/changed","params":{
        "threadId":"child","status":{"type":"idle"}}}),
    );
    assert_eq!(app.codex.panel.agents[0]["status"]["type"], "idle");
    stop(&mut app);
    tick(&peer, &mut app);
    let index = peer.controls.borrow().len() - 1;
    answer(
        &peer,
        &mut app,
        index,
        json!({"data":[{"id":"done","status":"completed"}]}),
    );
    assert!(app.codex.panel.stop.is_none());
    app.codex
        .panel
        .processes
        .push(crate::app::codex_panel_state::Process {
            thread: "child".into(),
            data: json!({"processId":"88","itemId":"exec","command":"sleep60","cwd":"/tmp"}),
        });
    app.codex.panel.selected = Some(PanelId::Process {
        item: Some("exec".into()),
        thread: "child".into(),
        process: "88".into(),
    });
    stop(&mut app);
    tick(&peer, &mut app);
    assert_eq!(
        peer.controls.borrow().last().expect("terminate"),
        &(
            "thread/backgroundTerminals/terminate".into(),
            json!({"threadId":"child","processId":"88"})
        )
    );
    assert!(peer
        .controls
        .borrow()
        .iter()
        .all(|(method, _)| !method.ends_with("/clean")));
}
