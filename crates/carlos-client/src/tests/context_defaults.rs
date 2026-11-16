//! Match host-default replies and private writes without mutating live usage or retaining composer text.

use carlos_proto::commands::Command;
use carlos_proto::context_defaults::{ContextChoice, ContextDefaults, ContextPolicy};
use carlos_proto::setting_commands::SetContext;
use carlos_proto::submissions::OperationState;
use serde_json::json;

use super::client::{attached, command, reply};
use super::session;
use crate::{ClientError, ReadReply};

#[test]
fn context_read_returns_its_original_request_id_and_leaves_shared_facts_untouched() {
    let (mut client, connection) = attached();
    let outgoing = client
        .read(connection, Command::ContextRead { session: session() })
        .expect("host default read");
    let effects = reply(&mut client, &outgoing, json!(defaults(Some(500_000))));
    assert_eq!(effects.replies[0].id, Some(outgoing.id()));
    assert!(
        matches!(&effects.replies[0].reply, ReadReply::ContextDefaults(defaults)
        if defaults.saved_tokens == Some(500_000) && defaults.policy == ContextPolicy::Owned)
    );
    let shared = client.session(&session()).expect("existing facts");
    assert_eq!(shared.info().context, None);
    assert!(shared.settings().is_none());
}

#[test]
fn context_writes_use_existing_private_operations_without_capturing_composer_text() {
    let (mut client, connection) = attached();
    let outgoing = client
        .submit(connection, |operation| Command::ContextSet {
            session: session(),
            request: SetContext {
                operation,
                choice: ContextChoice::Tokens(500_000),
            },
        })
        .expect("explicit host write");
    let Command::ContextSet {
        request: SetContext {
            operation, choice, ..
        },
        ..
    } = command(&outgoing).command
    else {
        panic!("typed context write");
    };
    assert_eq!(choice, ContextChoice::Tokens(500_000));
    assert_eq!(
        client
            .operations()
            .get(&operation)
            .expect("private operation")
            .text(),
        None
    );
    let _ = reply(
        &mut client,
        &outgoing,
        json!({"operation":operation, "session":session(),
        "state":{"kind":"accepted", "entry":null, "notice":"Saved context default"}}),
    );
    assert!(matches!(
        &client
            .operations()
            .get(&operation)
            .expect("private result")
            .state,
        OperationState::Accepted {
            notice: Some(_),
            ..
        }
    ));
}

#[test]
fn malformed_context_replies_cannot_become_auto_or_replace_another_sessions_default() {
    let mut foreign = defaults(None);
    foreign.session = "f".repeat(32).parse().expect("other session");
    assert!(matches!(
        crate::context_defaults::reply(session(), json!(foreign)),
        Err(ClientError::ForeignSession)
    ));
    for tokens in [99_999, 1_000_001] {
        assert!(crate::context_defaults::reply(session(), json!(defaults(Some(tokens)))).is_err());
    }
    let mut read_only = defaults(Some(500_000));
    for policy in [ContextPolicy::AttachedCodex, ContextPolicy::Unsupported] {
        read_only.policy = policy;
        assert!(crate::context_defaults::reply(session(), json!(read_only)).is_err());
    }
    let mut missing = json!(defaults(None));
    missing
        .as_object_mut()
        .expect("reply object")
        .remove("saved_tokens");
    assert!(crate::context_defaults::reply(session(), missing).is_err());
}

fn defaults(saved_tokens: Option<u64>) -> ContextDefaults {
    ContextDefaults {
        session: session(),
        saved_tokens,
        policy: ContextPolicy::Owned,
    }
}
