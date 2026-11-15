//! Prove independent child cursors, exact reader scope and one shared retained-data allowance.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::codex::panel_state::VerifiedOwners;
use super::codex::{panel_activity, panel_events, panel_receive};
use super::state::SessionState;
use super::task_activity::ActivityContext;
use super::task_activity_reads::activity_key;

fn session() -> SessionState {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.enable_codex();
    session.codex_panel.agents = vec![
        json!({"id":"child","source":{"subAgent":{}},"parentThreadId":"root","name":"Native child","status":{"type":"idle"}}),
    ];
    session.codex_panel.ownership =
        VerifiedOwners::new("root", session.codex_scope().epoch(), ["child".into()])
            .expect("verified ancestry");
    session.codex_panel.refreshed = Some(Instant::now());
    session
}

fn open(session: &mut SessionState, reader: Arc<AtomicBool>) -> ActivityContext {
    let page = session
        .task_page(None, None, 1)
        .expect("selected child page");
    session
        .open_task_activity(reader, page.tasks[0].id.clone(), &page.revision, None)
        .expect("reader admitted")
}

fn entry(index: usize) -> Value {
    json!({"turnId":"child-turn","item":{"id":format!("item-{index}"),"type":"agentMessage","text":format!("CHILD_ITEM_{index}")}})
}

fn reply(session: &mut SessionState, context: &ActivityContext, data: Value, cursor: Value) {
    let request = session.codex_panel.activities[&activity_key(&context.activity)]
        .request
        .clone()
        .expect("original native request");
    let scope = session.backend_context().panel;
    let frame = json!({"requestContext":{"operation":"panel","root":"root","epoch":scope.native.epoch,"token":request.token},
        "requestMethod":request.method,"requestParams":request.params,"response":{"result":{"data":data,"nextCursor":cursor}}});
    assert!(panel_receive::reply(
        session,
        scope,
        &mut Default::default(),
        &frame
    ));
}

#[test]
fn two_readers_on_the_same_child_keep_older_cursors_and_pages_independent() {
    let mut session = session();
    let first = open(&mut session, Arc::new(AtomicBool::new(true)));
    let second = open(&mut session, Arc::new(AtomicBool::new(true)));
    let data: Vec<_> = (0..20).map(entry).collect();
    reply(&mut session, &second, json!(data), json!("older"));
    reply(&mut session, &first, json!(data), json!("older"));
    let before = session
        .task_activity_header(&second)
        .expect("second header");
    let bp = session
        .task_activity_page(&second, &before.revision, None, 128)
        .expect("second page");
    session
        .start_activity_read(&activity_key(&first.activity), true)
        .expect("first reads older");
    reply(&mut session, &first, json!([entry(99)]), Value::Null);
    let after = session
        .task_activity_header(&first)
        .expect("first advanced");
    assert_eq!((after.items, before.items), (21, 20));
    assert_eq!(
        session
            .task_activity_header(&second)
            .expect("second unchanged"),
        before
    );
    let ap = session
        .task_activity_page(&first, &after.revision, None, 128)
        .expect("older first page");
    assert_eq!(ap.records[0].text, "CHILD_ITEM_99");
    assert_eq!(bp.records[0].text, "CHILD_ITEM_19");
    assert!(session
        .task_activity_page(&second, &before.revision, Some(&ap.records[0].id), 128)
        .is_err());
    let foreign = ActivityContext {
        activity: first.activity.clone(),
        reader: second.reader.clone(),
    };
    assert!(session.task_activity_header(&foreign).is_err());
    session.close_task_activities(&first.reader, None);
    assert!(session.task_activity_header(&first).is_err());
    assert!(session.task_activity_header(&second).is_ok());
}

#[test]
fn live_tail_changes_do_not_invalidate_record_pages_or_enter_the_root_transcript() {
    let mut session = session();
    let context = open(&mut session, Arc::new(AtomicBool::new(true)));
    reply(&mut session, &context, json!([entry(0)]), Value::Null);
    let before = session
        .task_activity_header(&context)
        .expect("accepted reader");
    let root_len = session.messages.len();
    let scope = session.backend_context().panel;
    assert!(panel_events::event(
        &mut session,
        scope,
        "item/agentMessage/delta",
        &json!({"threadId":"child","delta":"PRIVATE_CHILD_LIVE🌍"})
    ));
    let after = session.task_activity_header(&context).expect("live reader");
    assert_eq!(after.revision, before.revision);
    assert!(after.change > before.change);
    assert_eq!(after.tail, "PRIVATE_CHILD_LIVE🌍");
    assert_eq!(session.messages.len(), root_len);
    assert!(session
        .task_activity_page(&context, &before.revision, None, 128)
        .is_ok());
    session.adopt_native_thread("replacement".into());
    assert!(session.task_activity_header(&context).is_err());
}

#[test]
fn all_child_readers_share_one_native_eight_mib_allowance() {
    let mut session = session();
    let first = open(&mut session, Arc::new(AtomicBool::new(true)));
    let second = open(&mut session, Arc::new(AtomicBool::new(true)));
    let mut large = entry(0);
    large["item"]["text"] = json!("x".repeat(6 * 1024 * 1024));
    reply(&mut session, &first, json!([large]), Value::Null);
    let header = session
        .task_activity_header(&first)
        .expect("first accepted");
    let page = session
        .task_activity_page(&first, &header.revision, None, 128)
        .expect("bounded scalar");
    assert_eq!(page.records[0].text.len(), 32 * 1024);
    assert!(!page.records[0].complete);
    let mut too_large = entry(1);
    too_large["item"]["text"] = json!("y".repeat(3 * 1024 * 1024));
    reply(&mut session, &second, json!([too_large]), Value::Null);
    let refused = session
        .task_activity_header(&second)
        .expect("private error");
    assert!(!refused.known);
    assert!(refused.error.expect("shared bound").contains("8 MiB"));
    assert_eq!(
        session
            .task_activity_header(&first)
            .expect("first unchanged"),
        header
    );
    assert!(
        session
            .codex_panel
            .activities
            .values()
            .map(|activity| activity.bytes + activity.tail.len())
            .sum::<usize>()
            <= 8 * 1024 * 1024
    );
}

#[test]
fn malformed_pages_do_not_install_partial_records_under_an_old_revision() {
    let mut session = session();
    let context = open(&mut session, Arc::new(AtomicBool::new(true)));
    let key = activity_key(&context.activity);
    let malformed = json!({"data":[entry(0),{"turnId":"turn","item":{"type":"agentMessage"}}],"nextCursor":null});
    assert!(panel_activity::page(&mut session, &key, &malformed).is_err());
    assert!(session.codex_panel.activities[&key].items.is_empty());
    assert_eq!(session.codex_panel.activities[&key].bytes, 0);
    context.reader.store(false, Ordering::Release);
    session.expire_task_activities();
    assert!(session.task_activity_header(&context).is_err());
}

#[test]
fn cancelled_native_reads_stay_bounded_until_their_original_deadline() {
    let mut session = session();
    let reader = Arc::new(AtomicBool::new(true));
    for _ in 0..8 {
        open(&mut session, reader.clone());
        session.codex_panel.outgoing.clear();
    }
    session.close_task_activities(&reader, None);
    session.close_task_activities(&reader, None);
    reader.store(false, Ordering::Release);
    session.expire_task_activities();
    assert_eq!(session.codex_panel.activities.len(), 8);
    let page = session
        .task_page(None, None, 1)
        .expect("unchanged selected child");
    assert!(session
        .open_task_activity(
            Arc::new(AtomicBool::new(true)),
            page.tasks[0].id.clone(),
            &page.revision,
            None
        )
        .is_err());
    for activity in session.codex_panel.activities.values_mut() {
        activity.started = Instant::now() - Duration::from_secs(21);
    }
    session.expire_task_activities();
    assert!(session.codex_panel.activities.is_empty());
    open(&mut session, Arc::new(AtomicBool::new(true)));
}
