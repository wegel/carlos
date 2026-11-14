//! Keep several remote sessions within one device cache without losing submitted text or native work.

use carlos_proto::commands::Command;
use serde_json::json;

use super::client::{command, greet, reply};
use super::{entry, session, snapshot};
use crate::Client;

#[test]
fn combined_fact_limits_detach_only_the_overflowing_window_and_preserve_private_submitted_text() {
    let mut client = Client::default();
    let (connection, _) = greet(&mut client, false, "d");
    client
        .submit(connection, |operation| Command::Send {
            session: session(),
            operation,
            text: "private draft remains".into(),
            policy: Default::default(),
        })
        .expect("locally pending user text");
    let mut rejected = 0;
    for index in 1..=32 {
        let page = session_page(index, 500);
        let effects = adopt(&mut client, connection, page);
        if !effects.failures.is_empty() {
            rejected += 1;
            assert_eq!(effects.failures[0].error.code, -32010);
            assert!(effects
                .outgoing
                .iter()
                .all(|outgoing| matches!(command(outgoing).command, Command::Unsubscribe { .. })));
        }
    }
    assert!(
        rejected > 0,
        "per-session budgets cannot multiply into a gigabyte device cache"
    );
    let bytes: usize = client
        .sessions
        .values()
        .filter_map(|slot| slot.replica.as_ref())
        .map(|replica| replica.retained_bytes())
        .sum();
    assert!(bytes <= 64 * 1024 * 1024);
    assert_eq!(
        client
            .operations()
            .iter()
            .next()
            .expect("pending operation")
            .text(),
        Some("private draft remains")
    );
    let first = format!("{:032x}", 1).parse().expect("first session");
    assert!(
        client.session(&first).is_some(),
        "another session's accepted rows remain available"
    );
}

#[test]
fn future_history_pages_share_one_budget_and_overflow_preserves_all_live_windows() {
    let mut client = Client::default();
    let (connection, _) = greet(&mut client, false, "d");
    let mut rejected = 0;
    for index in 1..=5 {
        let page = session_page(index, 1);
        let id = page.info.id.clone();
        let before = page.entries[0].id.clone();
        adopt(&mut client, connection, page);
        let read = client
            .read(
                connection,
                Command::History {
                    session: id.clone(),
                    before,
                    limit: 500,
                },
            )
            .expect("bounded older read");
        let entries: Vec<_> = (0..500)
            .map(|row| entry(index * 1000 + row, &"x".repeat(4096)))
            .collect();
        let effects = reply(
            &mut client,
            &read,
            json!({"entries":entries,"has_older":false,"seq":"50"}),
        );
        rejected += effects.failures.len();
        assert_eq!(
            client
                .session(&id)
                .expect("live window preserved")
                .sequence()
                .get(),
            10
        );
        assert_eq!(
            client
                .session(&id)
                .expect("unmodified live rows")
                .entries()
                .count(),
            1
        );
    }
    assert!(
        rejected > 0,
        "pending future pages stop at the shared eight-MiB bound"
    );
}

fn session_page(index: u64, rows: u64) -> carlos_proto::session::Snapshot {
    let mut page = snapshot();
    page.info.id = format!("{index:032x}")
        .parse()
        .expect("independent session");
    page.entries = (0..rows)
        .map(|row| entry(index * 1000 + 500 + row, &"x".repeat(4096)))
        .collect();
    page
}

fn adopt(
    client: &mut Client,
    connection: crate::ConnectionId,
    page: carlos_proto::session::Snapshot,
) -> crate::Effects {
    let subscribe = client
        .read(
            connection,
            Command::Subscribe {
                session: page.info.id.clone(),
                since: None,
            },
        )
        .expect("session window");
    reply(
        client,
        &subscribe,
        json!({"kind":"snapshot", "snapshot":page}),
    )
}
