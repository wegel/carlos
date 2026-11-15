//! Keep task pages independent and reject addresses, runs and text reads from retired native facts.

use std::time::Instant;

use carlos_proto::bytes::ByteOffset;
use carlos_proto::ids::{TaskId, TaskRevision};
use carlos_proto::session::Backend;
use carlos_proto::tasks::TaskField;
use serde_json::json;

use super::codex::panel_state::{Process, VerifiedOwners};
use super::state::SessionState;

fn codex() -> SessionState {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.enable_codex();
    session.codex_panel.agents = (0..40)
        .map(|index| {
            json!({"id":format!("native-{index}"),
        "name":format!("Agent {index}"),"status":{"type":"idle"},
        "lastObservedTurn":{"id":"turn-one","status":"inProgress"}})
        })
        .collect();
    session.codex_panel.ownership = VerifiedOwners::new(
        "root",
        session.codex_scope().epoch(),
        (0..40).map(|index| format!("native-{index}")),
    )
    .expect("verified descendants");
    session.codex_panel.refreshed = Some(Instant::now());
    session
}

#[test]
fn clients_page_without_changing_the_shared_native_window_and_reject_old_preconditions() {
    let mut session = codex();
    let first = session.task_page(None, None, 1000).expect("newest page");
    assert_eq!(first.tasks.len(), 32);
    assert!(first.has_more);
    let second = session
        .task_page(Some(&first.revision), Some(&first.tasks[31].id), 32)
        .expect("other client page");
    assert_eq!(second.tasks.len(), 8);
    assert!(!second.has_more);
    assert_eq!(second.tasks[0].label, "Agent 32");
    assert_eq!(
        session
            .task_page(None, None, 32)
            .expect("first client unchanged"),
        first
    );
    assert!(session
        .task_page(None, Some(&first.tasks[0].id), 1)
        .is_err());
    let encoded = serde_json::to_string(&first).expect("typed task page");
    assert!(!encoded.contains("native-"));
    assert!(!encoded.contains("turn-one"));
    session.codex_panel.agents[0]["status"]["type"] = json!("future-native-state");
    assert!(session
        .task_detail(&first.revision, &first.tasks[0].id)
        .is_err());
    assert!(session
        .task_page(Some(&first.revision), Some(&first.tasks[31].id), 32)
        .is_err());
    let new = session.task_page(None, None, 1).expect("new facts");
    assert_eq!(new.tasks[0].id, first.tasks[0].id);
    assert_eq!(new.tasks[0].state, "future-native-state");
}

#[test]
fn task_ids_outlive_metadata_but_runs_and_connection_addresses_never_redirect() {
    let mut session = codex();
    let first = session.task_page(None, None, 1).expect("first run");
    session.codex_panel.agents[0]["name"] = json!("Renamed agent");
    let renamed = session.task_page(None, None, 1).expect("same run");
    assert_eq!(renamed.tasks[0].id, first.tasks[0].id);
    assert_eq!(renamed.tasks[0].run, first.tasks[0].run);
    session.codex_panel.agents[0]["lastObservedTurn"]["id"] = json!("replacement-turn");
    let replacement = session.task_page(None, None, 1).expect("replacement run");
    assert_eq!(replacement.tasks[0].id, first.tasks[0].id);
    assert_ne!(replacement.tasks[0].run, first.tasks[0].run);
    session.advance_codex_epoch();
    assert!(session
        .task_page(None, None, 32)
        .expect("old ownership retired")
        .tasks
        .is_empty());
    session.verify_codex_agent_owners_for_test(session.codex_scope().epoch());
    let current = session.task_page(None, None, 1).expect("new connection");
    assert_ne!(current.tasks[0].id, first.tasks[0].id);
    assert!(session
        .task_detail(&current.revision, &first.tasks[0].id)
        .is_err());
    session.adopt_native_thread("fork".into());
    assert!(session
        .task_detail(&current.revision, &current.tasks[0].id)
        .is_err());
}

#[test]
fn process_fields_keep_exact_unicode_bytes_with_small_previews_and_revision_checked_seeks() {
    let mut session = codex();
    session.codex_panel.agents.clear();
    let text = "🌍\0line\n".repeat(100000);
    session.codex_panel.processes.push(Process { thread: "root".into(), data:
        json!({"processId":"native-process","itemId":"item-one","command":text,"osPid":9007199254740993u64}) });
    let page = session.task_page(None, None, 32).expect("process page");
    assert!(!page.tasks[0].label_complete);
    let id = &page.tasks[0].id;
    let detail = session
        .task_detail(&page.revision, id)
        .expect("bounded detail");
    let command = detail
        .properties
        .iter()
        .find(|property| property.field == TaskField::Command)
        .expect("command preview");
    assert!(!command.complete);
    assert_eq!(command.bytes.get(), text.len() as u64);
    assert!(serde_json::to_vec(&detail).expect("escaped preview").len() < 512 * 1024);
    assert_eq!(
        detail
            .properties
            .iter()
            .find(|property| property.field == TaskField::OsPid)
            .expect("pid")
            .text,
        "9007199254740993"
    );
    let rounded = session
        .task_text(
            &page.revision,
            id,
            TaskField::Command,
            ByteOffset::new(2),
            4,
        )
        .expect("rounded Unicode seek");
    assert_eq!(rounded.offset.get(), 0);
    assert_eq!(rounded.text, "🌍");
    assert_eq!(complete_command(&session, &page.revision, id), text);
    session.codex_panel.processes[0].data["itemId"] = json!("replacement-item");
    let replacement = session.task_page(None, None, 1).expect("new process run");
    assert_eq!(replacement.tasks[0].id, *id);
    assert_ne!(replacement.tasks[0].run, page.tasks[0].run);
    assert!(session
        .task_text(
            &page.revision,
            id,
            TaskField::Command,
            ByteOffset::new(0),
            4
        )
        .is_err());
}

#[test]
fn claude_run_replacement_preserves_task_address_and_does_not_export_unknown_json_fields() {
    let mut session = SessionState::new("claude-root".into(), Default::default(), false);
    session.set_client_identity(Backend::Claude, "host".into());
    session.enable_claude();
    session
        .update_claude_task(&json!({"subtype":"task_started","task_id":"native-claude",
        "run_id":"a","description":"Child 🌍","usage":{"total_tokens":9007199254740993u64},
        "future":{"private":"not part of the typed task protocol"}}))
        .expect("native edge");
    let first = session.task_page(None, None, 32).expect("Claude tasks");
    assert!(first.known);
    assert_eq!(first.tasks[0].state, "running");
    let detail = session
        .task_detail(&first.revision, &first.tasks[0].id)
        .expect("Claude metadata");
    assert!(detail
        .properties
        .iter()
        .any(|property| property.field == TaskField::UsageTokens
            && property.text == "9007199254740993"));
    assert!(!serde_json::to_string(&detail)
        .expect("typed fields")
        .contains("future"));
    session.update_claude_task(&json!({"subtype":"task_started","task_id":"native-claude","run_id":"b","description":"New run"})).expect("replacement edge");
    let next = session.task_page(None, None, 32).expect("new run");
    assert_eq!(next.tasks[0].id, first.tasks[0].id);
    assert_ne!(next.tasks[0].run, first.tasks[0].run);
    assert!(session
        .task_detail(&first.revision, &first.tasks[0].id)
        .is_err());
}

#[test]
fn claude_process_task_identity_survives_native_reset_but_retires_with_its_helper() {
    let mut session = SessionState::new("before-clear".into(), Default::default(), false);
    session.set_client_identity(Backend::Claude, "host".into());
    session.enable_claude();
    session.set_claude_generation_for_test(1);
    session
        .update_claude_task(&json!({"subtype":"task_started","task_id":"child","run_id":"run-a"}))
        .expect("native task");
    let before = session
        .task_page(None, None, 1)
        .expect("original task page");
    session.set_thread_id("after-clear".into());
    let reset = session
        .task_page(None, None, 1)
        .expect("same helper after native clear");
    assert_eq!(reset.tasks[0].id, before.tasks[0].id);
    assert_eq!(reset.tasks[0].run, before.tasks[0].run);
    assert_ne!(
        reset.revision, before.revision,
        "the native transcript context did change"
    );
    session.set_claude_generation_for_test(2);
    let replaced = session.task_page(None, None, 1).expect("new helper");
    assert_ne!(replaced.tasks[0].id, before.tasks[0].id);
    assert_ne!(replaced.tasks[0].run, before.tasks[0].run);
    assert!(session
        .task_detail(&replaced.revision, &before.tasks[0].id)
        .is_err());
}

fn complete_command(session: &SessionState, revision: &TaskRevision, id: &TaskId) -> String {
    let mut offset = ByteOffset::new(0);
    let mut exact = String::new();
    loop {
        let chunk = session
            .task_text(revision, id, TaskField::Command, offset, 65536)
            .expect("exact chunk");
        assert_eq!(chunk.offset, offset);
        assert!(chunk.text.len() <= 65536);
        exact.push_str(&chunk.text);
        offset = chunk.next_offset;
        if chunk.complete {
            return exact;
        }
    }
}
