//! Enumerate changed-entry reports, no-ops, grouped reads, and full replacements.

use serde_json::json;

use crate::backend::BackendKind;

use super::approval_state::{ApprovalRequestDraft, ApprovalRequestKind};

use super::entry_key::{BackendEntryKey, NativeEntryKey};
use super::history_identity::HistoryMode;
use super::models::{EntryDraft, MessageKind, Role};
use super::state::SessionState;
use super::transcript_updates::SessionUpdate;

const ROLES: [Role; 7] = [
    Role::User,
    Role::Assistant,
    Role::Commentary,
    Role::Reasoning,
    Role::ToolCall,
    Role::ToolOutput,
    Role::System,
];

#[test]
fn every_row_edit_reports_heads_earliest_dirty_entry() {
    for previous in [
        "→ Read src/a.rs",
        "→ Read src/b.rs",
        "",
        "plain",
        "→ Read src/a.rs\nother",
    ] {
        for text in [None, Some("→ Read src/a.rs"), Some("plain"), Some("")] {
            for role in ROLES {
                for kind in [
                    MessageKind::Plain,
                    MessageKind::Diff,
                    MessageKind::UserSteer,
                ] {
                    for index in 0..=3 {
                        check_edit(previous, text, role, kind, index);
                    }
                }
            }
        }
    }
}

fn check_edit(previous: &str, text: Option<&str>, role: Role, kind: MessageKind, index: usize) {
    let mut session = SessionState::new("thread".into());
    session.append_message(Role::ToolCall, previous);
    session.append_message(Role::ToolCall, "");
    session.append_message(Role::Assistant, "old");
    let update = session.update_message_at_index(index, role, text.map(str::to_string), kind, None);
    let grouped = index > 0
        && index < 3
        && previous == "→ Read src/a.rs"
        && role == Role::ToolCall
        && kind == MessageKind::Plain
        && text == Some("→ Read src/a.rs");
    let expected = if index >= 3 {
        None
    } else {
        Some(session.messages[if grouped { 0 } else { index }].id)
    };
    assert_eq!(
        update,
        SessionUpdate {
            dirty_from: expected,
            ..SessionUpdate::default()
        }
    );
}

#[test]
fn every_stream_completion_and_replacement_reports_only_its_existing_dirty_calls() {
    for backend in [BackendKind::Codex, BackendKind::Claude, BackendKind::Grok] {
        for role in ROLES {
            for complete in [false, true] {
                for mapped in [false, true] {
                    check_stream(backend, role, complete, mapped);
                }
            }
        }
    }
    for mode in [HistoryMode::SameThread, HistoryMode::NewConversation] {
        for count in 0..8 {
            let mut session = SessionState::new("thread".into());
            for _ in 0..count {
                session.append_message(Role::Assistant, "old");
            }
            let update = session.replace_history(Vec::new(), mode);
            assert_eq!(
                update,
                SessionUpdate {
                    transcript_reset: true,
                    ..SessionUpdate::default()
                }
            );
            assert!(session.messages.is_empty());
        }
    }
}

fn check_stream(backend: BackendKind, role: Role, complete: bool, mapped: bool) {
    let mut session = SessionState::new("thread".into());
    let key = BackendEntryKey {
        backend,
        conversation: "thread".into(),
        turn: None,
        item: NativeEntryKey::Item("item".into()),
        part: 0,
    };
    session.append_message(Role::User, "prefix");
    if mapped {
        let (index, _) = session.append_draft_message(EntryDraft::plain(role, "old"));
        session.put_item_mapping(&key, index);
    }
    if complete {
        session.mark_item_completed(&key);
    }
    let update = session.upsert_agent_delta(&key, "new");
    assert_eq!(
        update.dirty_from,
        (!complete).then(|| session.messages[1].id)
    );
    assert!(!update.transcript_reset);
    if !complete {
        let (_, update) = session.upsert_mapped_message(
            &key,
            role,
            "replacement".into(),
            MessageKind::Plain,
            None,
        );
        assert_eq!(update.dirty_from, Some(session.messages[1].id));
    }
    let update = session.update_message_at_index(100, role, None, MessageKind::Plain, None);
    assert_eq!(update, SessionUpdate::default());
}

#[test]
fn empty_tail_and_repeated_markers_preserve_incremental_reports() {
    for count in 0..8 {
        for cut in 0..=count {
            let mut session = SessionState::new("thread".into());
            for _ in 0..count {
                session.append_message(Role::Assistant, "row");
            }
            assert_eq!(session.truncate_at(cut), cut);
            assert_eq!(session.messages.len(), cut);
            let first = session.append_turn_interrupted_marker();
            assert_eq!(first.dirty_from, Some(session.messages[cut].id));
            assert_eq!(
                session.append_turn_interrupted_marker(),
                SessionUpdate::default()
            );
            assert_eq!(
                session.append_context_compacted_marker().dirty_from,
                Some(session.messages[cut + 1].id)
            );
            assert_eq!(
                session.append_context_compacted_marker(),
                SessionUpdate::default()
            );
        }
    }
}

#[test]
fn changing_any_request_fact_assigns_a_new_card_id() {
    for field in 0..9 {
        let mut session = SessionState::new("thread".into());
        let _ = session.set_pending_approval(approval_draft());
        let original = session.approval().pending().expect("original").entry_id();
        let mut copied = session.approval().pending().expect("original").to_draft();
        match field {
            0 => copied.request_id = json!("different"),
            1 => copied.method = "different/method".into(),
            2 => copied.kind = ApprovalRequestKind::ClaudeQuestion,
            3 => copied.title = "Different title".into(),
            4 => copied.detail_lines.push("different details".into()),
            5 => {
                copied.requested_permissions = Some(json!({"toolName":"Bash",
                "input":{"command":"echo different"},"defaultToNo":true}))
            }
            6 => copied.can_accept_for_session = true,
            7 => copied.can_decline = false,
            8 => copied.can_cancel = false,
            _ => unreachable!(),
        }
        assert!(
            !copied.same_request(&approval_draft()),
            "changed field {field}"
        );
        assert_eq!(session.set_pending_approval(copied), vec![original]);
        assert_ne!(
            session.approval().pending().expect("new").entry_id(),
            original
        );
    }
}

#[test]
fn identical_drafts_keep_only_the_active_card_and_queue_moves_keep_their_own_ids() {
    use super::approval_state::ApprovalQueue;
    let mut session = SessionState::new("thread".into());
    let _ = session.set_pending_approval(approval_draft());
    let original = session.approval().pending().expect("original").entry_id();
    assert!(session.set_pending_approval(approval_draft()).is_empty());
    assert_eq!(
        session.approval().pending().expect("duplicate").entry_id(),
        original
    );
    let queued = session.enqueue_approval(approval_draft(), ApprovalQueue::Claude);
    assert_ne!(queued, original);
    session.focus_approval(queued, ApprovalQueue::Claude);
    assert_eq!(
        session.approval().pending().expect("focused").entry_id(),
        queued
    );
    assert_eq!(
        session.approval().queued(ApprovalQueue::Claude)[0].entry_id(),
        original
    );
}

#[test]
fn retired_ids_and_previous_namespaces_cannot_restore_consent() {
    use super::approval_state::ApprovalQueue;
    let mut session = SessionState::new("thread".into());
    let _ = session.set_pending_approval(approval_draft());
    let retired = session.approval().pending().expect("original").entry_id();
    assert_eq!(session.retire_approval(retired), vec![retired]);
    assert!(session.retire_approval(retired).is_empty());
    session.focus_approval(retired, ApprovalQueue::Claude);
    assert!(session.approval().pending().is_none());
    let _ = session.set_pending_approval(approval_draft());
    let fresh = session.approval().pending().expect("new card").entry_id();
    assert_ne!(fresh, retired);
    session.reset_for_forked_thread();
    session.focus_approval(retired, ApprovalQueue::Codex);
    assert_eq!(
        session.approval().pending().expect("still live").entry_id(),
        fresh
    );
    assert!(session.approval_answer(retired).is_none());
}

fn approval_draft() -> ApprovalRequestDraft {
    ApprovalRequestDraft {
        request_id: json!("one"),
        method: "claude/native".into(),
        kind: ApprovalRequestKind::ClaudeNative,
        title: "Allow tool?".into(),
        detail_lines: vec!["echo original".into()],
        requested_permissions: Some(json!({"toolName":"Bash",
            "input":{"command":"echo original"},"defaultToNo":true})),
        can_accept_for_session: false,
        can_decline: true,
        can_cancel: true,
    }
}
