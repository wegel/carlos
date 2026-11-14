//! Exercise client data with fixed wire envelopes and no native process, socket or clock.

mod cards;
mod codec;
mod replica;
mod windows;

use carlos_proto::ids::{EntryId, Sequence, SessionId};
use carlos_proto::session::{Entry, Snapshot};
use serde_json::json;

pub(super) fn session() -> SessionId {
    "a".repeat(32).parse().expect("session")
}
pub(super) fn entry_id(index: u64) -> EntryId {
    format!("{}-{index:016x}", "b".repeat(32))
        .parse()
        .expect("entry")
}
pub(super) fn entry(index: u64, text: &str) -> Entry {
    serde_json::from_value(json!({"id":entry_id(index),"kind":"answer","text":text,"done":false,"tool":null,"operation":null})).expect("entry")
}
pub(super) fn snapshot() -> Snapshot {
    serde_json::from_value(json!({"info":{"id":session(),"name":null,"backend":"codex","model":null,"host":"test","cwd":null,"phase":"idle","availability":"ready","current_turn":null,"ralph":false,"turn_started_at":null,"context":null,"quota":null},"entries":[entry(10,"hello")],"approvals":[],"seq":"10","has_older":true})).expect("snapshot")
}
pub(super) fn event(
    seq: u64,
    event: carlos_proto::events::Event,
) -> carlos_proto::events::SessionEvent {
    carlos_proto::events::SessionEvent {
        session: session(),
        seq: Sequence::new(seq),
        event,
    }
}
