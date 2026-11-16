//! Keep native body handles host-bound and preserve checked cursors, UTF-8 bytes and old lifetimes.

use std::sync::Arc;

use serde_json::json;

use crate::test_support::{connected, info, reply, request, SESSION};
use crate::{PhoneClient, PhoneError};

const ENTRY: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb-0000000000000001";
const ORIGINAL: &str = "a🌍\u{1b}[31mred\u{1b}[0m\n";

fn body_client() -> Arc<PhoneClient> {
    let client = connected();
    let frame = client.subscribe(SESSION.into()).expect("subscribe");
    reply(
        &client,
        &frame,
        json!({"kind":"snapshot","snapshot":{
            "info":info("ready"),"seq":"1","has_older":false,"approvals":[],"entries":[{
                "id":ENTRY,"kind":"answer","text":"","done":true,"tool":null,"operation":null,
                "source":{"revision":"7","bytes":ORIGINAL.len().to_string()}
            }]
        }}),
    );
    client
}

#[test]
fn original_source_keeps_exact_bytes_and_exports_only_bounded_utf8_slices() {
    let client = body_client();
    let body = client
        .open_source(SESSION.into(), ENTRY.into(), true)
        .expect("source");
    let frame = client.read_body(body.clone()).expect("read");
    assert_eq!(request(&frame)["method"], "entry/source");
    let batch = reply(
        &client,
        &frame,
        json!({
            "entry":ENTRY,"offset":"0","source":{"revision":"7","bytes":ORIGINAL.len().to_string()},
            "from":null,"next":"source-next","text":ORIGINAL,"has_more":false,"done":true,"seq":"1"
        }),
    );
    assert_eq!(batch.bodies.len(), 1);
    assert_eq!(batch.bodies[0].entry(), ENTRY);
    let window = client.source_window(body.clone()).expect("source progress");
    assert_eq!(window.start, 0);
    assert_eq!(window.end, ORIGINAL.len() as u64);
    assert!(window.done && !window.has_more);
    assert_eq!(
        client
            .source_text(body.clone(), 0, 65536)
            .expect("exact copy"),
        ORIGINAL
    );
    assert_eq!(
        client.source_text(body.clone(), 0, 3).expect("UTF-8 floor"),
        "a"
    );
    assert!(matches!(
        client.source_text(body.clone(), 2, 4),
        Err(PhoneError::InvalidInput { .. })
    ));
    assert!(matches!(
        client.source_text(body, 0, 65537),
        Err(PhoneError::InvalidInput { .. })
    ));
}

#[test]
fn a_foreign_host_cannot_read_watch_or_close_an_identical_body_address() {
    let owner = body_client();
    let other = body_client();
    let body = owner
        .open_source(SESSION.into(), ENTRY.into(), false)
        .expect("owner source");
    let other_body = other
        .open_source(SESSION.into(), ENTRY.into(), false)
        .expect("other source");
    assert!(matches!(
        other.read_body(body.clone()),
        Err(PhoneError::InvalidInput { .. })
    ));
    assert!(matches!(
        other.watch_body(body.clone()),
        Err(PhoneError::InvalidInput { .. })
    ));
    assert!(matches!(
        other.source_window(body.clone()),
        Err(PhoneError::InvalidInput { .. })
    ));
    assert!(matches!(
        other.close_body(body.clone()),
        Err(PhoneError::InvalidInput { .. })
    ));
    assert!(owner.source_window(body).is_ok());
    assert!(other.source_window(other_body).is_ok());
}

#[test]
fn closing_an_old_handle_preserves_the_replacement_and_current_close_returns_unsubscribe() {
    let client = body_client();
    let old = client
        .open_source(SESSION.into(), ENTRY.into(), false)
        .expect("old source");
    let current = client
        .open_source(SESSION.into(), ENTRY.into(), false)
        .expect("replacement");
    client.close_body(old.clone()).expect("retire old reader");
    assert!(matches!(
        client.source_window(old),
        Err(PhoneError::StaleRead { .. })
    ));
    assert!(client.source_window(current.clone()).is_ok());
    let watch = client.watch_body(current.clone()).expect("watch");
    assert_eq!(request(&watch)["method"], "entry/source/subscribe");
    let batch = client.close_body(current.clone()).expect("close current");
    assert_eq!(batch.outgoing.len(), 1);
    assert_eq!(
        request(&batch.outgoing[0])["method"],
        "entry/source/unsubscribe"
    );
    assert!(matches!(
        client.read_body(current),
        Err(PhoneError::StaleRead { .. })
    ));
}
