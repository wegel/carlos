//! Keep equal text from different clients distinct through native echo and later row updates.

use carlos_proto::events::Event;
use carlos_proto::submissions::ClientOperation;
use serde_json::{json, Value};

use super::state::SessionState;

fn operation(client: char) -> ClientOperation {
    ClientOperation {
        client: client.to_string().repeat(32).parse().expect("client"),
        id: "1".parse().expect("counter"),
    }
}

fn item(state: &mut SessionState, native: &str, token: Option<&str>) {
    let value = json!({"method":"item/completed","params":{"threadId":"root","turnId":"turn",
        "item":{"type":"userMessage","id":native,"clientId":token,"content":[{"type":"text","text":"same text"}]}}});
    state.handle_backend_line(&value.to_string(), &state.backend_context());
}

#[test]
fn equal_native_prompts_belong_to_their_exact_registered_tokens() {
    let mut state = SessionState::new("root".into(), Default::default(), false);
    state.enable_codex();
    let first = operation('a');
    let second = operation('b');
    let mut params = Value::Null;
    state.prepare_operation_token(&mut params, Some(&first));
    let first_token = params["clientUserMessageId"]
        .as_str()
        .expect("token")
        .to_owned();
    state.prepare_operation_token(&mut params, Some(&second));
    let second_token = params["clientUserMessageId"]
        .as_str()
        .expect("token")
        .to_owned();
    item(&mut state, "other-native-client", Some("unknown-token"));
    item(&mut state, "second", Some(&second_token));
    item(&mut state, "first", Some(&first_token));
    let snapshot = state.publish().snapshot(100);
    assert_eq!(snapshot.entries.len(), 3);
    assert_eq!(snapshot.entries[0].operation, None);
    assert_eq!(snapshot.entries[1].operation, Some(second));
    assert_eq!(snapshot.entries[2].operation, Some(first));
}

#[test]
fn a_delayed_client_token_updates_the_same_row_and_reaches_new_snapshots() {
    let mut state = SessionState::new("root".into(), Default::default(), false);
    state.enable_codex();
    let owner = operation('a');
    let mut params = Value::Null;
    state.prepare_operation_token(&mut params, Some(&owner));
    item(&mut state, "native", None);
    let first = state.publish().snapshot(100).entries.remove(0);
    assert!(first.operation.is_none());
    item(&mut state, "native", params["clientUserMessageId"].as_str());
    let published = state.publish();
    let events: Vec<_> = published
        .records()
        .iter()
        .filter_map(|record| published.to_wire(record))
        .collect();
    assert!(events.iter().any(|event| matches!(&event.event, Event::EntryUpdated { entry } if entry.id == first.id && entry.operation.as_ref() == Some(&owner))));
    assert_eq!(published.snapshot(100).entries[0].operation, Some(owner));
    assert_eq!(published.entry_count(), 1);
}
