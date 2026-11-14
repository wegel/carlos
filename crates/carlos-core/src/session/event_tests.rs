//! Reconstruct a headless client's state from sealed JSON events.

use carlos_proto::events::{EntryFacts, Event, SessionEvent};
use carlos_proto::ids::Sequence;
use carlos_proto::session::{Decision, LabelSource, Phase, Snapshot};
use serde_json::{from_str, json, to_string};

use super::approval_state::{ApprovalRequestDraft, ApprovalRequestKind};
use super::events::Change;
use super::history_identity::HistoryMode;
use super::models::{EntryDraft, MessageKind, Role};
use super::runtime_settings::RuntimeDefaults;
use super::state::SessionState;

fn session() -> SessionState {
    SessionState::new("native-root".into(), RuntimeDefaults::default(), false)
}

fn apply(client: &mut Snapshot, event: SessionEvent) {
    assert_eq!(client.info.id, event.session);
    assert_eq!(client.seq.checked_next(), Some(event.seq));
    match event.event {
        Event::EntryAdded { entry } => client.entries.push(entry),
        Event::EntryUpdated { entry } => {
            let old = client
                .entries
                .iter_mut()
                .find(|old| old.id == entry.id)
                .expect("live row");
            *old = entry;
        }
        Event::TextAppended { entry, text } => {
            client
                .entries
                .iter_mut()
                .find(|old| old.id == entry)
                .expect("live row")
                .text
                .push_str(&text);
        }
        Event::EntryFactsChanged { entry, facts } => apply_facts(client, entry, facts),
        Event::OutputChanged {
            entry,
            revision,
            has_output,
        } => {
            apply_output_facts(client, entry, revision, has_output);
        }
        Event::SourceChanged { entry, source } => {
            apply_source(client, entry, source);
        }
        Event::ApprovalRequested { approval, before } => {
            client.approvals.retain(|old| old.id != approval.id);
            let index = before
                .and_then(|id| client.approvals.iter().position(|old| old.id == id))
                .unwrap_or(client.approvals.len());
            client.approvals.insert(index, approval);
        }
        Event::ApprovalResolved { approval, .. } => {
            client.approvals.retain(|old| old.id != approval)
        }
        Event::InfoChanged { info } => client.info = info,
        Event::Reset { snapshot } => {
            assert_eq!(snapshot.seq, event.seq);
            *client = snapshot;
        }
    }
    client.seq = event.seq;
}

fn apply_facts(client: &mut Snapshot, entry: carlos_proto::ids::EntryId, facts: EntryFacts) {
    let row = client
        .entries
        .iter_mut()
        .find(|old| old.id == entry)
        .expect("live row");
    row.kind = facts.kind;
    row.format = facts.format;
    row.file_path = facts.file_path;
    row.done = facts.done;
    row.tool = facts.tool;
}

fn apply_source(
    client: &mut Snapshot,
    entry: carlos_proto::ids::EntryId,
    source: carlos_proto::source::Source,
) {
    let row = client
        .entries
        .iter_mut()
        .find(|row| row.id == entry)
        .expect("live row");
    row.text.clear();
    row.source = Some(source);
}

fn apply_output_facts(
    client: &mut Snapshot,
    entry: carlos_proto::ids::EntryId,
    revision: carlos_proto::output::OutputRevision,
    has_output: bool,
) {
    let tool = client
        .entries
        .iter_mut()
        .find(|row| row.id == entry)
        .expect("live row")
        .tool
        .as_mut()
        .expect("tool");
    tool.output_revision = revision;
    tool.has_output = has_output;
}

pub(super) fn synchronize(session: &mut SessionState, client: &mut Snapshot) -> Vec<SessionEvent> {
    let published = session.publish();
    let events: Vec<_> = published
        .records()
        .iter()
        .map(|record| {
            let wire = published.to_wire(record).expect("sealed event");
            from_str::<SessionEvent>(&to_string(&wire).expect("encode")).expect("decode")
        })
        .collect();
    for event in events.iter().cloned() {
        apply(client, event);
    }
    assert_eq!(*client, published.snapshot(100));
    events
}

#[test]
fn client_reconstructs_unicode_deltas_role_changes_and_completion() {
    let mut core = session();
    let mut client = core.publish().snapshot(100);
    let key = core.backend_entry_key("secret-native-item");
    core.upsert_agent_delta(&key, "hé");
    synchronize(&mut core, &mut client);
    core.upsert_agent_delta(&key, "llo ");
    core.upsert_agent_delta(&key, "🌍");
    let events = synchronize(&mut core, &mut client);
    assert!(events
        .iter()
        .any(|event| matches!(&event.event, Event::TextAppended { text, .. } if text == "llo 🌍")));
    assert!(!to_string(&events)
        .expect("encode")
        .contains("secret-native"));
    core.messages.complete(&key);
    let events = synchronize(&mut core, &mut client);
    assert!(events
        .iter()
        .any(|event| matches!(event.event, Event::EntryFactsChanged { .. })));
    assert!(client.entries[0].done);
    let id = core.messages[0].id;
    core.messages.update(
        0,
        Role::Commentary,
        Some("replacement".into()),
        MessageKind::Plain,
        None,
    );
    core.messages.append_text(0, "!", None);
    synchronize(&mut core, &mut client);
    assert_eq!(core.messages[0].id, id);
    assert_eq!(client.entries[0].text, "replacement!");
    assert!(core.publish().records().is_empty());
}

#[test]
fn resets_preserve_session_clock_and_retire_old_addresses() {
    let mut core = session();
    core.append_message(Role::User, "old");
    let mut client = core.publish().snapshot(100);
    let old = client.entries[0].id.clone();
    let session = client.info.id.clone();
    core.reset_for_forked_thread();
    core.append_message(Role::User, "new");
    let events = synchronize(&mut core, &mut client);
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0].event, Event::Reset { .. }));
    assert_eq!(client.info.id, session);
    assert!(core.publish().history(&old, 100).is_none());
    assert_ne!(client.entries[0].id, old);
    core.messages.adopt_unkeyed(
        vec![EntryDraft::plain(Role::Assistant, "history")],
        HistoryMode::SameThread,
    );
    synchronize(&mut core, &mut client);
}

fn approval() -> ApprovalRequestDraft {
    ApprovalRequestDraft {
        request_id: json!("secret-native-request"),
        method: "native-method".into(),
        kind: ApprovalRequestKind::CommandExecution,
        title: "Run tests".into(),
        detail_lines: vec!["cargo test".into()],
        requested_permissions: None,
        can_accept_for_session: true,
        can_decline: true,
        can_cancel: true,
    }
}

#[test]
fn approvals_emit_requests_submission_and_retirement_without_native_routes() {
    let mut core = session();
    let mut client = core.publish().snapshot(100);
    let _ = core.set_pending_approval(approval());
    let id = core.approval.pending().expect("card").entry_id();
    let events = synchronize(&mut core, &mut client);
    assert_eq!(client.approvals.len(), 1);
    assert!(!to_string(&events)
        .expect("encode")
        .contains("secret-native-request"));
    assert!(core.begin_approval_answer(id));
    synchronize(&mut core, &mut client);
    assert!(client.approvals[0].submitted);
    let _ = core.retire_approval(id);
    let events = synchronize(&mut core, &mut client);
    assert!(events
        .iter()
        .any(|e| matches!(e.event, Event::ApprovalResolved { .. })));
    let _ = core.retire_approval(id);
    assert!(core.publish().records().is_empty());
}

#[test]
fn large_bursts_are_bounded_and_snapshot_pages_have_valid_cursors() {
    let mut core = session();
    core.publish();
    for _ in 0..6001 {
        core.append_message(Role::User, "row");
    }
    let batch = core.publish();
    assert_eq!(batch.records().len(), 1);
    assert!(matches!(
        batch.records()[0].change,
        Change::Reset {
            dirty_from: Some(_)
        }
    ));
    let snapshot = batch.snapshot(10);
    assert!(snapshot.has_older);
    assert_eq!(snapshot.entries.len(), 10);
    let page = batch.history(&snapshot.entries[0].id, 20).expect("older");
    assert_eq!(page.entries.len(), 20);
    assert_eq!(page.seq, snapshot.seq);
    assert_eq!(snapshot.seq, Sequence::new(2));
}

#[test]
fn overflow_of_existing_row_edits_requests_one_reset_and_keeps_earliest_hint() {
    let mut core = session();
    for _ in 0..300 {
        core.append_message(Role::User, "old");
    }
    core.publish();
    for i in (0..300).rev() {
        core.messages.append_text(i, "!", None);
    }
    let batch = core.publish();
    assert_eq!(batch.records().len(), 1);
    assert!(
        matches!(batch.records()[0].change, Change::Reset { dirty_from: Some(id) } if batch.position(id) == Some(0))
    );
}

#[test]
fn tool_events_do_not_send_output_until_requested() {
    let mut core = session();
    let mut client = core.publish().snapshot(100);
    core.append_message(Role::ToolOutput, "command\nprivate output");
    synchronize(&mut core, &mut client);
    assert_eq!(client.entries[0].text, "");
    let batch = core.publish();
    let output = batch
        .output_chunk(&client.entries[0].id, None, 1024)
        .expect("output");
    assert_eq!(output.segments[0].text, "command\nprivate output");
}

#[test]
fn normalized_reasoning_sends_only_the_new_unicode_suffix() {
    let mut core = session();
    let mut client = core.publish().snapshot(100);
    let key = core.backend_entry_key("reason");
    core.upsert_reasoning_delta(&key, "héllo");
    core.normalize_reasoning_at(0);
    synchronize(&mut core, &mut client);
    core.upsert_reasoning_delta(&key, " 🌍");
    core.normalize_reasoning_at(0);
    let events = synchronize(&mut core, &mut client);
    assert!(events
        .iter()
        .any(|e| matches!(&e.event, Event::TextAppended { text, .. } if text == " 🌍")));
    assert!(!events
        .iter()
        .any(|e| matches!(e.event, Event::EntryUpdated { .. })));
}

#[test]
fn header_changes_and_tool_labels_reach_the_client_without_new_rows() {
    let mut core = session();
    let mut client = core.publish().snapshot(100);
    core.set_client_identity(carlos_proto::session::Backend::Claude, "workstation".into());
    core.set_thread_name(Some("Work".into()));
    core.set_runtime_settings(Some("model".into()), None, None);
    core.set_context_usage(Some(super::context_usage::ContextUsage {
        used: 12,
        max: 100,
    }));
    core.record_turn_started("native-turn");
    synchronize(&mut core, &mut client);
    assert_eq!(client.info.phase, Phase::Running);
    assert!(client.info.turn_started_at.is_some());
    core.append_message(Role::ToolOutput, "output");
    synchronize(&mut core, &mut client);
    core.messages.set_tool(
        0,
        super::models::ToolOutputDetails {
            generated: true,
            summary: "Run tests".into(),
            failed: false,
        },
    );
    let events = synchronize(&mut core, &mut client);
    assert_eq!(client.entries.len(), 1);
    assert_eq!(
        client.entries[0].tool.as_ref().expect("tool").label_source,
        LabelSource::Ai
    );
    assert!(events
        .iter()
        .any(|e| matches!(e.event, Event::EntryFactsChanged { .. })));
}

#[test]
fn accepted_decision_is_distinct_from_backend_retirement() {
    let mut core = session();
    let mut client = core.publish().snapshot(100);
    let _ = core.set_pending_approval(approval());
    synchronize(&mut core, &mut client);
    let id = core.approval.pending().expect("card").entry_id();
    core.record_approval_decision(id, super::approval_state::ApprovalChoice::AcceptForSession);
    let _ = core.retire_approval(id);
    let events = synchronize(&mut core, &mut client);
    assert!(events.iter().any(|e| matches!(
        e.event,
        Event::ApprovalResolved {
            decision: Some(Decision::Session),
            ..
        }
    )));
}
