//! Exercise reader ownership, shared byte bounds and exact process proofs for captured task windows.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use carlos_proto::ids::TaskWindowId;
use carlos_proto::submissions::ClientOperation;
use carlos_proto::task_windows::{MAX_NATIVE_TASK_BYTES, MAX_TASK_WINDOWS};
use serde_json::{json, Value};

use super::codex::{panel_process_runs, panel_runs};
use super::state::SessionState;
use super::task_window_state::Contents;
use super::task_windows::WindowContext;

fn session() -> SessionState {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.enable_codex();
    session.codex_panel.refreshed = Some(Instant::now());
    session.codex_panel.older = json!("older:256");
    session
}

fn open(session: &mut SessionState, reader: Arc<AtomicBool>) -> WindowContext {
    let revision = session
        .task_page(None, None, 1)
        .expect("latest page")
        .revision;
    let header = session
        .open_task_window(reader.clone(), &revision, None)
        .expect("window admitted");
    WindowContext {
        reader,
        window: header.window,
    }
}

fn page(session: &mut SessionState, id: &TaskWindowId, result: Value) {
    let Contents::Reading(read) = &session.task_windows.slots[id].contents else {
        panic!("pending window")
    };
    let params = json!({"requestContext":{"token":read.request.token},
        "requestMethod":read.request.method,"requestParams":read.request.params,"response":{"result":result}});
    assert!(session.receive_task_window(&params));
}

fn capture(session: &mut SessionState, reader: Arc<AtomicBool>, process: bool) -> WindowContext {
    let context = open(session, reader);
    page(
        session,
        &context.window,
        json!({"data":[{"id":"older-child","source":{"subAgent":{}},"parentThreadId":"root", "name":"Frozen older child", "status":{"type":"notLoaded"}}],"nextCursor":null}),
    );
    let rows = if process {
        json!([{"processId":"original","itemId":"item","osPid":123,"command":"sleep 🌍"}])
    } else {
        json!([])
    };
    page(
        session,
        &context.window,
        json!({"data":rows,"nextCursor":null}),
    );
    assert!(
        session
            .read_task_window(&context)
            .expect("captured header")
            .known
    );
    context
}

#[test]
fn older_windows_preserve_latest_facts_and_check_the_original_reader() {
    let mut session = session();
    let latest = session.task_page(None, None, 1).expect("latest");
    let first = capture(&mut session, Arc::new(AtomicBool::new(true)), false);
    let second = capture(&mut session, Arc::new(AtomicBool::new(true)), false);
    assert_ne!(first.window, second.window);
    assert_eq!(
        session.task_page(None, None, 1).expect("latest unchanged"),
        latest
    );
    let header = session.read_task_window(&first).expect("first header");
    let before = session
        .task_page_in(Some(&first), Some(&header.revision), None, 32)
        .expect("first page");
    assert_eq!(before.tasks[0].label, "Frozen older child");
    session
        .codex_panel
        .agents
        .push(json!({"id":"unrelated","name":"latest child"}));
    session.task_metadata_changed();
    assert_eq!(
        session
            .task_page_in(Some(&first), Some(&header.revision), None, 32)
            .expect("frozen page"),
        before
    );
    let stolen = WindowContext {
        window: first.window.clone(),
        reader: second.reader.clone(),
    };
    assert!(session.read_task_window(&stolen).is_err());
    session.close_task_windows(&first.reader, None);
    assert!(session.read_task_window(&first).is_err());
    assert!(session.read_task_window(&second).is_ok());
    second.reader.store(false, Ordering::Release);
    session.expire_task_windows();
    assert!(session.task_windows.slots.is_empty());
}

#[test]
fn window_admission_and_parser_share_the_existing_metadata_allowance() {
    let mut session = session();
    session.codex_panel.reports.insert(
        "large".into(),
        json!("x".repeat(MAX_NATIVE_TASK_BYTES - 1024)),
    );
    session.task_metadata_changed();
    let context = open(&mut session, Arc::new(AtomicBool::new(true)));
    page(
        &mut session,
        &context.window,
        json!({"data":[{"id":"too-large","name":"x".repeat(2048)}]}),
    );
    let failed = session
        .read_task_window(&context)
        .expect("explicit failure");
    assert!(!failed.known);
    assert!(failed
        .read_error
        .expect("budget error")
        .contains("shared 8 MiB"));
    assert!(session.codex_panel.agents.is_empty());
    assert!(session.task_window_bytes() <= MAX_NATIVE_TASK_BYTES);
}

#[test]
fn latest_metadata_growth_retires_older_cache_without_erasing_latest_facts() {
    let mut session = session();
    let context = capture(&mut session, Arc::new(AtomicBool::new(true)), false);
    session.codex_panel.agents =
        vec![json!({"id":"root-child", "name":"x".repeat(MAX_NATIVE_TASK_BYTES - 128)})];
    session.task_metadata_changed();
    session.enforce_task_window_bytes();
    assert!(session.task_window_bytes() <= MAX_NATIVE_TASK_BYTES);
    let header = session
        .read_task_window(&context)
        .expect("explicit cache retirement");
    assert!(!header.known);
    assert!(header
        .read_error
        .expect("retirement reason")
        .contains("Latest native task metadata grew"));
    assert_eq!(session.codex_panel.agents.len(), 1);
    assert!(session.task_known());
}

#[test]
fn native_metadata_changes_during_an_older_read_do_not_publish_a_mixed_window() {
    let mut session = session();
    let context = open(&mut session, Arc::new(AtomicBool::new(true)));
    page(
        &mut session,
        &context.window,
        json!({"data":[],"nextCursor":null}),
    );
    session.codex_panel.revision += 1;
    page(
        &mut session,
        &context.window,
        json!({"data":[],"nextCursor":null}),
    );
    let header = session
        .read_task_window(&context)
        .expect("original read identity");
    assert!(!header.known);
    assert!(header
        .read_error
        .expect("explicit mixed-read failure")
        .contains("changed while reading"));
    assert!(session.task_known());
}

#[test]
fn retired_inflight_reads_keep_the_window_count_bound_until_reply_or_original_deadline() {
    let mut session = session();
    let reader = Arc::new(AtomicBool::new(true));
    let contexts: Vec<_> = (0..MAX_TASK_WINDOWS)
        .map(|_| {
            let context = open(&mut session, reader.clone());
            session.codex_panel.outgoing.clear(); // This request has reached the native writer.
            context
        })
        .collect();
    session.close_task_windows(&reader, None);
    assert_eq!(session.task_windows.slots.len(), MAX_TASK_WINDOWS);
    session.close_task_windows(&reader, None);
    reader.store(false, Ordering::Release);
    session.expire_task_windows();
    assert_eq!(
        session.task_windows.slots.len(),
        MAX_TASK_WINDOWS,
        "repeated close and EOF do not forget native reads still in flight"
    );
    reader.store(true, Ordering::Release);
    let revision = session.task_page(None, None, 1).expect("latest").revision;
    assert!(session
        .open_task_window(reader.clone(), &revision, None)
        .is_err());
    for slot in session.task_windows.slots.values_mut() {
        let Contents::Retired { started, .. } = &mut slot.contents else {
            panic!("retired request")
        };
        *started = Instant::now() - Duration::from_secs(21);
    }
    session.expire_task_windows();
    assert!(session.task_windows.slots.is_empty());
    assert!(session.read_task_window(&contexts[0]).is_err());
    assert!(session.open_task_window(reader, &revision, None).is_ok());
}

#[test]
fn older_process_stop_requires_a_fresh_matching_item_and_pid() {
    let mut session = session();
    let context = capture(&mut session, Arc::new(AtomicBool::new(true)), true);
    let page = session
        .task_page_in(Some(&context), None, None, 32)
        .expect("older process page");
    let process = &page.tasks[1];
    let operation: ClientOperation =
        serde_json::from_value(json!({"client":"a".repeat(32),"id":"1"})).expect("operation");
    assert!(session
        .prepare_task_stop_in(
            Some(context.clone()),
            operation.clone(),
            process.id.clone(),
            process.run.clone().expect("captured identity")
        )
        .is_err());
    reject_replacement_pid(&mut session, &context, process);
    session
        .prepare_task_run_in(Some(&context), &process.id, None, true)
        .expect("explicit retry");
    let key = panel_process_runs::key(&process.id);
    panel_process_runs::page(
        &mut session,
        &key,
        &json!({"data":[{"processId":"original","itemId":"item","osPid":123}],"nextCursor":null}),
    )
    .expect("same native identity");
    let accepted = session
        .task_run_snapshot_in(Some(&context), &process.id)
        .expect("verified original process");
    assert!(accepted.known);
    assert_eq!(accepted.run, process.run);
    let work = session
        .prepare_task_stop_in(
            Some(context),
            operation,
            process.id.clone(),
            accepted.run.expect("fresh run"),
        )
        .expect("original process stop prepared");
    let scope = session.backend_context().panel;
    super::codex::task_status::observed(
        &mut session,
        scope,
        "item/started",
        &json!({
        "threadId":"root", "item":{"type":"commandExecution","id":"item"}}),
    );
    assert!(
        !work.claim.live.load(Ordering::SeqCst),
        "an older owner's native item change retires a queued writer proof even without processId"
    );
}

fn reject_replacement_pid(
    session: &mut SessionState,
    context: &WindowContext,
    process: &carlos_proto::tasks::Task,
) {
    session
        .prepare_task_run_in(Some(context), &process.id, None, true)
        .expect("fresh PID proof");
    let key = panel_process_runs::key(&process.id);
    let original = session
        .task_run_snapshot_in(Some(context), &process.id)
        .expect("pending proof");
    assert!(original.reading);
    assert!(original.run.is_none());
    let changed =
        json!({"data":[{"processId":"original","itemId":"item","osPid":456}],"nextCursor":null});
    let error =
        panel_process_runs::page(session, &key, &changed).expect_err("replacement PID rejected");
    panel_runs::failed(session, &key, &error);
    assert!(session
        .task_run_snapshot_in(Some(context), &process.id)
        .expect("private failure")
        .error
        .expect("mismatch")
        .contains("not retargeted"));
}
