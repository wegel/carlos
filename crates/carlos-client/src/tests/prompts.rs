//! Reject crossed prompt reads and false byte cursors before a platform edits its composer.

use carlos_proto::bytes::ByteOffset;
use carlos_proto::commands::Command;
use carlos_proto::prompts::{PromptDirection, PromptSource};
use serde_json::json;

use super::client::{attached, reply};
use super::{entry_id, session};
use crate::codec::Incoming;
use crate::ReadReply;

#[test]
fn selecting_an_exact_entry_rejects_another_record_with_the_same_text_length() {
    for wrong_entry in [None, Some(entry_id(11))] {
        let (mut client, connection) = attached();
        let request = client
            .read(
                connection,
                Command::PromptSelect {
                    session: session(),
                    source: PromptSource::Human,
                    anchor: Some(entry_id(10)),
                    direction: PromptDirection::AtEntry,
                },
            )
            .expect("select an exact visible entry");
        assert!(client.receive(connection, Incoming::Success {
            id: request.id(),
            result: json!({"session":session(),"source":"human","has_older":false,"has_newer":false,
                "record":{"id":entry_id(20),"entry":wrong_entry,"source":"human","bytes":"4"}}),
        }).is_err());
    }
}

#[test]
fn prompt_selection_and_exact_text_use_the_original_typed_request() {
    let (mut client, connection) = attached();
    let request = client
        .read(
            connection,
            Command::PromptSelect {
                session: session(),
                source: PromptSource::Unknown,
                anchor: None,
                direction: PromptDirection::Latest,
            },
        )
        .expect("explicit unknown history");
    let result = reply(
        &mut client,
        &request,
        json!({
            "session":session(),"source":"unknown","has_older":false,"has_newer":false,
            "record":{"id":entry_id(20),"entry":entry_id(20),"source":"unknown","bytes":"4"},
        }),
    );
    assert_eq!(result.replies[0].id, Some(request.id()));
    assert!(
        matches!(&result.replies[0].reply, ReadReply::PromptSelection(selection)
        if selection.record.as_ref().expect("record").id == entry_id(20))
    );
    let request = client
        .read(
            connection,
            Command::PromptText {
                session: session(),
                prompt: entry_id(20),
                offset: ByteOffset::default(),
                limit: Some(4),
            },
        )
        .expect("original text");
    let result = reply(
        &mut client,
        &request,
        json!({
            "session":session(),"prompt":entry_id(20),"offset":"0","total":"4","text":"🌍","next":null,
        }),
    );
    assert!(matches!(&result.replies[0].reply, ReadReply::PromptText(chunk) if chunk.text == "🌍"));
}

#[test]
fn prompt_text_rejects_other_records_offsets_false_eof_and_empty_continuations() {
    for wrong in [
        json!({"prompt":entry_id(21),"offset":"0","text":"🌍","total":"4","next":null}),
        json!({"prompt":entry_id(20),"offset":"1","text":"🌍","total":"5","next":null}),
        json!({"prompt":entry_id(20),"offset":"0","text":"🌍","total":"8","next":null}),
        json!({"prompt":entry_id(20),"offset":"0","text":"","total":"8","next":"0"}),
    ] {
        let (mut client, connection) = attached();
        let request = client
            .read(
                connection,
                Command::PromptText {
                    session: session(),
                    prompt: entry_id(20),
                    offset: ByteOffset::default(),
                    limit: Some(4),
                },
            )
            .expect("read target");
        let mut wrong = wrong;
        wrong["session"] = json!(session());
        assert!(client
            .receive(
                connection,
                Incoming::Success {
                    id: request.id(),
                    result: wrong
                }
            )
            .is_err());
    }
}
