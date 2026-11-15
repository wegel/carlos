//! Preserve native text and source facts previously checked through the terminal owner.

use serde_json::json;

use super::item_changes::ItemChanges;
use super::item_history::append_history_from_thread;
use super::item_receive::receive_reasoning;
use super::models::Role;
use super::notification_items::handle_item_notification;
use super::state::SessionState;

fn session() -> SessionState {
    SessionState::new("thread-1".into(), Default::default(), false)
}

#[test]
fn append_message_coalesces_successive_read_summaries() {
    let mut session = session();
    for text in [
        "→ Read src/main.rs [offset=10]",
        "→ Read src/main.rs",
        "→ Read src/main.rs [offset=30]",
        "→ Read src/lib.rs",
    ] {
        session.append_message(Role::ToolCall, text);
    }
    assert_eq!(session.messages().len(), 4);
    assert_eq!(session.messages()[0].text, "→ Read src/main.rs ×3");
    assert_eq!(session.messages()[1].text, "");
    assert_eq!(session.messages()[2].text, "");
    assert_eq!(session.messages()[3].text, "→ Read src/lib.rs");
}

#[test]
fn append_message_does_not_coalesce_read_summaries_across_other_messages() {
    let mut session = session();
    session.append_message(Role::ToolCall, "→ Read src/main.rs");
    session.append_message(Role::Assistant, "working");
    session.append_message(Role::ToolCall, "→ Read src/main.rs");
    assert_eq!(session.messages().len(), 3);
    assert_eq!(session.messages()[0].text, "→ Read src/main.rs");
    assert_eq!(session.messages()[1].text, "working");
    assert_eq!(session.messages()[2].text, "→ Read src/main.rs");
}

#[test]
fn set_command_override_coalesces_with_previous_read_summary() {
    let mut session = session();
    session.append_message(Role::ToolCall, "→ Read src/main.rs");
    let index = session.append_message(Role::ToolCall, "");
    let key = session.backend_entry_key("call-2");
    session.put_item_mapping(&key, index);
    session.set_command_override(&key, "→ Read src/main.rs [offset=40]".into());
    assert_eq!(session.messages()[0].text, "→ Read src/main.rs ×2");
    assert_eq!(session.messages()[1].text, "");
}

fn summary(parts: &[&str]) -> String {
    let mut session = session();
    let index = session.append_message(Role::Reasoning, "");
    let key = session.backend_entry_key("reason-1");
    session.put_item_mapping(&key, index);
    for part in parts {
        receive_reasoning(&mut session, "reason-1", part);
    }
    assert_eq!(session.item_message_index(&key), Some(index));
    session.messages()[index].text.clone()
}

#[test]
fn reasoning_summary_delta_inserts_newline_between_bold_chunks() {
    assert_eq!(
        summary(&["**First thought**", "**Second thought**"]),
        "**First thought**\n**Second thought**"
    );
}

#[test]
fn reasoning_summary_delta_trims_space_before_split_closing_marker() {
    assert_eq!(
        summary(&["**Preparing concise, evidence-based answer ", "**"]),
        "**Preparing concise, evidence-based answer**"
    );
}

#[test]
fn reasoning_summary_delta_normalizes_split_adjacent_bold_chunks() {
    assert_eq!(
        summary(&[
            "**Investigating display bug causes ",
            "**",
            " **Analyzing markdown rendering issues ",
            "**",
        ]),
        "**Investigating display bug causes**\n**Analyzing markdown rendering issues**"
    );
}

#[test]
fn read_metadata_reaches_live_and_resumed_transcripts() {
    let item = json!({"type":"toolOutput", "id":"read-1", "name":"Read", "input":{"file_path":"main.rs"}, "output":"1:let value = \"hello\";"});
    let mut live = session();
    let mut changes = ItemChanges::default();
    let params = json!({"item":item});
    assert!(handle_item_notification(
        &mut live,
        &mut changes,
        "item/completed",
        params.as_object().expect("fixed item"),
    ));
    let mut resumed = session();
    append_history_from_thread(&mut resumed, &json!({"turns":[{"items":[item]}]}));
    for session in [live, resumed] {
        assert_eq!(session.messages().len(), 1);
        assert_eq!(session.messages()[0].file_path.as_deref(), Some("main.rs"));
        assert_eq!(session.messages()[0].text, "1:let value = \"hello\";");
    }
}

#[test]
fn saved_command_reads_keep_source_path_command_and_status() {
    let mut session = session();
    append_history_from_thread(
        &mut session,
        &json!({"turns":[{"items":[{
            "type":"commandExecution", "id":"read", "command":"cat main.rs", "commandActions":[{"type":"read","command":"cat main.rs","path":"main.rs"}],
            "aggregatedOutput":"let value = \"hello\";", "exitCode":0
        }]}]}),
    );
    assert_eq!(session.messages().len(), 1);
    assert_eq!(session.messages()[0].file_path.as_deref(), Some("main.rs"));
    assert_eq!(
        session.messages()[0].text,
        "$ cat main.rs\nlet value = \"hello\";\nexit code: 0"
    );
}

#[test]
fn mapped_answer_delta_preserves_the_existing_entry_and_appends_text() {
    let mut session = session();
    session.append_message(Role::User, "prompt one");
    let index = session.append_message(Role::Assistant, "starting answer");
    let key = session.backend_entry_key("item-1");
    session.put_item_mapping(&key, index);
    let id = session.messages()[index].id;
    session.publish();
    session.upsert_agent_delta(
        &key,
        "\ncontinued answer with more text so the last message grows substantially",
    );
    assert_eq!(session.messages().len(), 2);
    assert_eq!(session.item_message_index(&key), Some(index));
    assert_eq!(session.messages()[index].id, id);
    assert_eq!(
        session.messages()[index].text,
        "starting answer\ncontinued answer with more text so the last message grows substantially"
    );
    assert!(session.publish().records().iter().any(|record| {
        matches!(record.change, super::events::Change::Text { entry, .. } if entry == id)
    }));
}
