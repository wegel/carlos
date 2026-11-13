//! Read reconnect task facts without constructing an app or any terminal widget.

use serde_json::json;

use super::panel_snapshot::reconnect_panel;
use crate::session::test_backend::TestBackend;

#[test]
fn reconnect_panel_reads_only_native_facts_without_a_view() {
    let peer = TestBackend::default();
    peer.reads.lock().expect("reads").extend([
        (
            "thread/list".into(),
            json!({"result":{"data":[],"nextCursor":null}}),
        ),
        (
            "thread/backgroundTerminals/list".into(),
            json!({"result":{"data":[],"nextCursor":null}}),
        ),
    ]);
    let panel = reconnect_panel(&peer, "root").expect("complete panel");
    assert_eq!(
        panel,
        json!({"agents":[],"processes":[],"older":null,"error":null,
        "notice":"All verified descendants in this snapshot","verifiedOwners":[],"processScopes":{"root":null}})
    );
    let calls = peer.calls.lock().expect("calls");
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].0, "thread/list");
    assert_eq!(calls[0].1["ancestorThreadId"], "root");
    assert_eq!(calls[0].1["limit"], 1);
    assert_eq!(calls[1].0, "thread/backgroundTerminals/list");
    assert_eq!(calls[1].1, json!({"threadId":"root","limit":1}));
    assert!(peer.reads.lock().expect("all replies consumed").is_empty());
}
