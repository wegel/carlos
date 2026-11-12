//! Check label guards and history-only requests without constructing a terminal view.

use serde_json::json;

use super::entry_key::{BackendEntryKey, NativeEntryKey};
use super::history_identity::{HistoryEntry, HistoryMode};
use super::models::{EntryDraft, MessageKind, Role};
use super::state::SessionState;
use crate::backend::BackendKind;
use crate::summaries::{command_key, SummaryEvent};

fn key() -> BackendEntryKey {
    BackendEntryKey {
        backend: BackendKind::Codex,
        conversation: "thread".into(),
        turn: Some("turn".into()),
        item: NativeEntryKey::Item("command".into()),
        part: 0,
    }
}

fn session(command: &str, live: bool) -> SessionState {
    let mut session = SessionState::new("thread".into(), Default::default(), true);
    session.upsert_mapped_message(
        &key(),
        Role::ToolOutput,
        "body".into(),
        MessageKind::Plain,
        None,
    );
    let item = json!({"id":"command","type":"commandExecution","command":command,"exitCode":0});
    session.observe_summary_item(&item, live, &key(), "thread:command".into());
    session
}

#[test]
fn a_result_must_match_both_the_current_key_and_the_current_binding() {
    for (same_key, same_binding) in [(false, false), (false, true), (true, false), (true, true)] {
        let mut session = session("python3 old.py", true);
        let old_binding = session
            .summary_binding_for_test(session.messages()[0].id)
            .expect("old binding");
        let changed = json!({"id":"command","type":"commandExecution","command":"python3 new.py"});
        session.observe_summary_item(&changed, true, &key(), "thread:command".into());
        let id = session.messages()[0].id;
        let current = session.summary_binding_for_test(id).expect("new binding");
        let result_key = command_key(if same_key {
            "python3 new.py"
        } else {
            "python3 old.py"
        });
        let binding = if same_binding { current } else { old_binding };
        let dirty = session.handle_summary_event(SummaryEvent::Ready(
            result_key,
            binding,
            Some("New report".into()),
        ));
        let tool = session.messages()[0]
            .tool_output
            .as_ref()
            .expect("tool label");
        assert_eq!(
            dirty,
            if same_key && same_binding {
                vec![id]
            } else {
                vec![]
            }
        );
        assert_eq!(tool.generated, same_key && same_binding);
        assert_eq!(
            tool.summary,
            if same_key && same_binding {
                "run New report"
            } else {
                "run python3 new.py"
            }
        );
        assert_eq!(session.messages()[0].text, "body");
    }
}

#[test]
fn history_requests_and_nested_replay_never_grant_model_work() {
    for (enabled, live, replaying, expected) in [
        (true, true, false, true),
        (true, false, false, false),
        (false, true, false, false),
        (true, true, true, false),
    ] {
        let mut session = SessionState::new("thread".into(), Default::default(), enabled);
        assert!(!session.set_summary_replaying(replaying));
        session.upsert_mapped_message(
            &key(),
            Role::ToolCall,
            "body".into(),
            MessageKind::Plain,
            None,
        );
        session.observe_summary_item(
            &json!({"id":"command","command":"python3 process.py"}),
            live,
            &key(),
            "thread:command".into(),
        );
        let binding = session
            .summaries
            .bindings
            .get(&session.messages()[0].id)
            .expect("request");
        assert_eq!(binding.request.generate, expected);
        assert_eq!(binding.request.entry, "thread:command");
        assert_eq!(session.set_summary_replaying(true), replaying);
        assert!(session.set_summary_replaying(replaying));
    }
}

#[test]
fn cache_entry_names_keep_native_turn_and_part_spelling() {
    let session = SessionState::new("thread".into(), Default::default(), true);
    for (item, scope, expected) in [
        ("command", None, "thread:command"),
        (
            "command",
            Some(("turn", 0)),
            "thread:codex:[\"turn\",\"command\",0]",
        ),
        (
            "x#part-1",
            Some(("a:b", 2)),
            "thread:codex:[\"a:b\",\"x#part-1\",2]",
        ),
    ] {
        assert_eq!(session.summary_cache_entry(item, scope), expected);
    }
}

#[test]
fn retired_entries_and_newly_ineligible_commands_ignore_old_results() {
    for retire in [false, true] {
        let mut session = session("python3 process.py", true);
        let token = session
            .summary_binding_for_test(session.messages()[0].id)
            .expect("binding");
        if retire {
            session.truncate_at(0);
        } else {
            session.observe_summary_item(
                &json!({"id":"command","command":"cat a.rs"}),
                true,
                &key(),
                "thread:command".into(),
            );
        }
        assert!(session
            .handle_summary_event(SummaryEvent::Ready(
                command_key("python3 process.py"),
                token,
                Some("STALE".into())
            ))
            .is_empty());
        assert!(session
            .messages()
            .iter()
            .all(|m| !m.tool_output.as_ref().is_some_and(|t| t.generated)));
    }
}

#[test]
fn session_history_replacement_retires_bindings_even_when_it_retains_entry_ids() {
    let mut session = session("python3 process.py", true);
    let id = session.messages()[0].id;
    let token = session.summary_binding_for_test(id).expect("old binding");
    session.replace_history(
        vec![HistoryEntry {
            occurrences: 1,
            key: Some(key()),
            draft: EntryDraft::plain(Role::ToolOutput, "replacement body"),
        }],
        HistoryMode::SameThread,
    );
    assert_eq!(session.messages()[0].id, id);
    assert!(session.summary_binding_for_test(id).is_none());
    assert!(session.summaries.pending.is_empty());
    assert!(session
        .handle_summary_event(SummaryEvent::Ready(
            command_key("python3 process.py"),
            token,
            Some("STALE".into()),
        ))
        .is_empty());
    assert_eq!(session.messages()[0].text, "replacement body");
    assert!(session.messages()[0].tool_output.is_none());
}
