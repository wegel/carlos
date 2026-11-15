//! Admit an overdue quota read once after real native snapshot adoption, never while rebuilding.
#![cfg(unix)]

use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tungstenite::{Message, WebSocket};

use super::reconnect_contract_support::{ready, session, snapshot};
use crate::backend::BackendClient;
use crate::codex_connection::CodexClient;
use crate::session::state::SessionState;
use crate::session::test_backend::TestBackend;

#[test]
fn due_quota_does_not_write_or_spin_before_ready_then_refreshes_once() {
    let directory = tempfile::tempdir().expect("private native endpoint");
    let path = directory.path().join("quota.sock");
    let listener = UnixListener::bind(&path).expect("listen");
    let (release, wait) = mpsc::channel();
    let server = thread::spawn(move || serve_quota(listener, wait));
    let mut peer = CodexClient::start(Some(path)).expect("actual native adapter");
    let mut state = session();
    let now = Instant::now();
    state.tick_rate_limits(&peer, now - Duration::from_secs(400));
    assert!(!state.backend_reads_ready());
    state.tick_rate_limits(&peer, now);
    assert_eq!(backend_wait(&state, now), None);
    let snapshot_peer = TestBackend::default();
    ready(&snapshot_peer, &mut state, snapshot("fresh", "idle"));
    state.set_codex_runtime_refresh_for_test(false);
    assert!(
        state.backend_reads_ready(),
        "closed={} rebuilding={} rows={:?}",
        state.codex_scope().closed(),
        state.codex_connection().rebuilding(),
        state
            .messages()
            .iter()
            .map(|row| row.text.as_str())
            .collect::<Vec<_>>()
    );
    state.tick_rate_limits(&peer, now);
    state.tick_rate_limits(&peer, now);
    assert!(backend_wait(&state, now).expect("future quota deadline") > Duration::ZERO);
    peer.call(
        "thread/read",
        json!({"threadId":"root"}),
        Duration::from_secs(3),
    )
    .expect("same native writer FIFO barrier");
    peer.stop();
    release.send(()).expect("release native fixture");
    let calls = server.join().expect("native fixture finishes");
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0]["method"], "account/rateLimits/read");
    assert_eq!(calls[0]["params"], json!({}));
    assert_eq!(calls[1]["method"], "thread/read");
}

fn backend_wait(state: &SessionState, now: Instant) -> Option<Duration> {
    let mut scope = state.backend_context().panel;
    scope.visible = false;
    state.backend_wait(now, scope, false)
}

fn serve_quota(listener: UnixListener, release: mpsc::Receiver<()>) -> Vec<Value> {
    let stream = listener.accept().expect("native connection").0;
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .expect("bounded reads");
    let mut socket = tungstenite::accept(stream).expect("native upgrade");
    let mut calls = Vec::new();
    loop {
        let request = next(&mut socket);
        let barrier = request["method"] == "thread/read";
        calls.push(request.clone());
        if barrier {
            socket
                .send(Message::Text(
                    json!({"id":request["id"],"result":{
                "thread":{"id":"root"}}})
                    .to_string()
                    .into(),
                ))
                .expect("barrier reply");
            release
                .recv_timeout(Duration::from_secs(3))
                .expect("fixture release");
            return calls;
        }
        assert_eq!(request["method"], "account/rateLimits/read");
    }
}

fn next(socket: &mut WebSocket<UnixStream>) -> Value {
    serde_json::from_str(
        socket
            .read()
            .expect("bounded native frame")
            .to_text()
            .expect("native text"),
    )
    .expect("native JSON")
}
