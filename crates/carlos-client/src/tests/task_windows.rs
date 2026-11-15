//! Keep captured task-window replies tied to their original session, handle and wire bounds.

use carlos_proto::commands::Command;
use carlos_proto::ids::TaskWindowId;
use serde_json::{json, Value};

use super::client::{attached, reply};
use super::session;
use crate::codec::Incoming;
use crate::{ClientError, ReadReply};

fn window() -> TaskWindowId {
    "d".repeat(64).parse().expect("captured window")
}

fn header() -> Value {
    json!({"session":session(),"window":window(),"seq":"10","revision":"e".repeat(64),
        "rows":256,"known":true,"reading":false,"older_available":true,"read_error":null})
}

#[test]
fn captured_window_reads_are_typed_private_views_and_never_replace_latest_facts() {
    let (mut client, connection) = attached();
    let before = client
        .session(&session())
        .expect("accepted session")
        .tasks()
        .cloned();
    let outgoing = client
        .read(
            connection,
            Command::TaskWindowRead {
                session: session(),
                window: window(),
            },
        )
        .expect("captured read");
    let effects = reply(&mut client, &outgoing, header());
    assert!(
        matches!(&effects.replies[0].reply, ReadReply::TaskWindow(header) if header.window == window() && header.rows == 256)
    );
    assert_eq!(
        client
            .session(&session())
            .expect("same authoritative replica")
            .tasks(),
        before.as_ref()
    );
}

#[test]
fn captured_window_reads_reject_foreign_handles_sessions_and_oversized_headers() {
    for field in ["session", "window", "rows", "known"] {
        let (mut client, connection) = attached();
        let outgoing = client
            .read(
                connection,
                Command::TaskWindowRead {
                    session: session(),
                    window: window(),
                },
            )
            .expect("original window read");
        let mut invalid = header();
        invalid[field] = match field {
            "session" => json!("f".repeat(32)),
            "window" => json!("f".repeat(64)),
            "rows" => json!(769),
            "known" => json!(false),
            _ => unreachable!("fixed negative cases"),
        };
        let error = client
            .receive(
                connection,
                Incoming::Success {
                    id: outgoing.id(),
                    result: invalid,
                },
            )
            .err()
            .expect("invalid header rejected");
        assert!(
            matches!(
                error,
                ClientError::ForeignSession | ClientError::InvalidPayload(_)
            ),
            "{field}"
        );
    }
}
