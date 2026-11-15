//! Clear an old edit badge only for a new send while its distinct recovery stays available.

use std::io::{ErrorKind, Read};
use std::os::unix::net::UnixStream;

use carlos_client::codec::Incoming;
use carlos_proto::submissions::{ClientOperation, OperationResult, OperationState};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use serde_json::Value;

use super::{device, recover, reject, transport};
use crate::app::remote::transport::Transport;

#[test]
fn a_successful_later_send_dismisses_the_failure_badge_but_keeps_saved_drafts() {
    let (mut device, connection) = device();
    device.set_composer("later composer");
    let edited = reject(&mut device, connection, "saved failed edit", true);
    device.history.operation = Some(edited.clone());
    assert!(device.apply_history_operation(&edited));
    assert!(device.history.failed);
    let (mut link, mut peer) = transport(connection);
    device
        .event(
            Some(&mut link),
            Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        )
        .expect("actual ordinary send path");
    let sent = sent_request(&mut link, &mut peer);
    assert_eq!(sent["method"], "session/send");
    assert_eq!(sent["params"]["text"], "later composer");
    let operation: ClientOperation = serde_json::from_value(sent["params"]["operation"].clone())
        .expect("actual new send origin");
    let result = OperationResult {
        operation,
        session: Some(device.session.clone()),
        rewound: false,
        state: OperationState::Accepted {
            entry: None,
            notice: None,
        },
    };
    device
        .receive(
            &mut link,
            Incoming::Success {
                id: serde_json::from_value(sent["id"].clone()).expect("actual request id"),
                result: serde_json::to_value(result).expect("matching successful send"),
            },
        )
        .expect("accepted ordinary send");
    assert!(!device.history.failed);
    assert_eq!(
        device.history.recoveries.iter().collect::<Vec<_>>(),
        [&edited]
    );
    assert!(device.view.input.is_empty());
    recover(&mut device);
    assert_eq!(device.view.input.lines().join("\n"), "saved failed edit");
    assert!(device.client.operations().get(&edited).is_none());
}

fn sent_request(link: &mut Transport, peer: &mut UnixStream) -> Value {
    link.flush().expect("write bounded local requests");
    peer.set_nonblocking(true).expect("read available bytes");
    let mut text = String::new();
    if let Err(error) = peer.read_to_string(&mut text) {
        assert_eq!(error.kind(), ErrorKind::WouldBlock);
    }
    let mut requests = text
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("actual socket envelope"))
        .filter(|request| request["method"] != "hello")
        .collect::<Vec<_>>();
    assert_eq!(requests.len(), 1, "one explicit ordinary send");
    requests.pop().expect("send request")
}
