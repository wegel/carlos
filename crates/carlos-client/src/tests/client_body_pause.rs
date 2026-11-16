//! Pause a platform formatter's source stream without losing its accepted bytes or cursor.

use carlos_proto::commands::Command;
use carlos_proto::events::Notification;
use serde_json::json;

use super::{source, tool, window};
use crate::tests::client::{attached, command, notify, reply};
use crate::tests::{entry_id, session};

#[test]
fn an_explicit_source_unsubscribe_preserves_the_checked_window_and_resumes_its_cursor() {
    let (mut client, connection) = attached();
    tool(&mut client, connection, 11, 3);
    let token = window(&mut client);
    let watching = client.watch_body(connection, &token).expect("source watch");
    let _ = reply(&mut client, &watching, source("raw", None, "accepted-end"));
    let paused = client
        .read(
            connection,
            Command::SourceUnsubscribe {
                session: session(),
                entry: entry_id(10),
            },
        )
        .expect("explicit bounded stream pause");
    let late = json!({"session":session(),"chunk":{"entry":entry_id(10),"offset":"3",
        "source":{"revision":"3","bytes":"9"},"text":"suffix","from":"accepted-end",
        "next":"new-end","has_more":false,"done":false,"seq":"12"}});
    let ignored = notify(
        &mut client,
        connection,
        Notification::Source(serde_json::from_value(late).expect("in-flight source")),
    );
    assert!(ignored.bodies.is_empty());
    let window = client.bodies().source(&token).expect("accepted bytes");
    assert_eq!(window.end().get(), 3);
    assert!(reply(&mut client, &paused, json!(null)).outgoing.is_empty());
    let resumed = client
        .watch_body(connection, &token)
        .expect("same checked window");
    assert!(matches!(command(&resumed).command,
        Command::SourceSubscribe { cursor: Some(cursor), .. } if cursor.as_str() == "accepted-end"));
    let accepted = reply(
        &mut client,
        &resumed,
        json!({"entry":entry_id(10),"offset":"3","source":{"revision":"3","bytes":"9"},
            "text":"suffix","from":"accepted-end","next":"new-end","has_more":false,
            "done":false,"seq":"12"}),
    );
    assert!(accepted.failures.is_empty());
    assert_eq!(
        client
            .bodies()
            .source(&token)
            .expect("exact resumed original")
            .pieces()
            .collect::<String>(),
        "rawsuffix"
    );
    assert_eq!(
        client.operations().iter().count(),
        0,
        "pausing never submits chat"
    );
}
