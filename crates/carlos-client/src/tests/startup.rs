//! Keep discovered native sessions private and recover open outcomes without replaying writes.

use carlos_proto::commands::Command;
use carlos_proto::session::Backend;
use carlos_proto::startup::{NativeLaunchOptions, OpenTarget, SavedPurpose};
use serde_json::json;

use super::client::{attached, command, greet, reply};
use super::session;
use crate::{ReadReply, Welcome};

#[test]
fn saved_session_pages_match_the_requested_backend_scope_and_metadata_limits() {
    let (mut client, connection) = attached();
    let outgoing = client
        .read(
            connection,
            Command::SavedOpen {
                backend: Backend::Claude,
                cwd: "/project".into(),
                codex_socket: None,
                purpose: SavedPurpose::Picker,
            },
        )
        .expect("native picker read");
    let page = json!({"scope":"b".repeat(32),"backend":"claude","rows":[{"choice":"c".repeat(32),
        "name":"Plain title", "preview":"Original é🦊", "cwd":"/project", "created_at":0,"updated_at":1}],
        "has_newer":false,"has_older":true,"can_archive":false});
    let effects = reply(&mut client, &outgoing, page.clone());
    assert!(
        matches!(&effects.replies[0].reply, ReadReply::SavedPage(page) if page.rows[0].preview == "Original é🦊")
    );
    let target = crate::startup::PageTarget {
        scope: Some("d".repeat(32).parse().expect("other scope")),
        backend: None,
    };
    assert!(crate::startup::reply(target, page.clone()).is_err());
    let mut oversized = page;
    oversized["rows"][0]["preview"] = json!("x".repeat(4097));
    assert!(crate::startup::reply(
        crate::startup::PageTarget {
            scope: None,
            backend: None
        },
        oversized
    )
    .is_err());
}

#[test]
fn admitted_open_uses_the_returned_logical_address_and_never_replays_its_native_import() {
    let (mut client, connection) = attached();
    let outgoing = client
        .submit(connection, |operation| Command::Open {
            operation,
            backend: Backend::Claude,
            cwd: "/project".into(),
            target: OpenTarget::Resume {
                address: "explicit-legacy-native".into(),
            },
            launch: NativeLaunchOptions::default(),
            tool_labels: None,
        })
        .expect("explicit native import");
    let Command::Open { operation, .. } = command(&outgoing).command else {
        panic!("typed import")
    };
    reply(
        &mut client,
        &outgoing,
        json!({"operation":operation,"session":session(),"state":{"kind":"pending"}}),
    );
    assert_eq!(
        client
            .operations()
            .get(&operation)
            .expect("private open")
            .session
            .as_ref(),
        Some(&session())
    );
    client.disconnect(connection);
    let (_, effects) = greet(&mut client, true, "d");
    assert_eq!(effects.welcome, Some(Welcome::Resumed));
    assert!(effects
        .outgoing
        .iter()
        .any(|outgoing| matches!(command(outgoing).command, Command::Operation { .. })));
    assert!(effects.outgoing.iter().all(|outgoing| !matches!(
        command(outgoing).command,
        Command::Open { .. } | Command::Create { .. }
    )));
}
