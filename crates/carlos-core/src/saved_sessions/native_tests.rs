//! Exercise bounded native catalogue requests and cancellation through a real Unix socket.

use std::os::unix::net::UnixListener;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use carlos_proto::session::Backend;
use serde_json::{json, Value};
use tungstenite::Message;

use super::{read_page, PageDirection, SavedPageRequest};
use crate::launch_cancel::LaunchCancellation;

#[test]
fn codex_reads_sorted_native_pages_without_opening_any_conversation() {
    let root = tempfile::tempdir().expect("private catalogue");
    let path = root.path().join("native.sock");
    let listener = UnixListener::bind(&path).expect("native socket");
    let server = thread::spawn(move || serve(listener, None));
    let page = read_page(request(root.path(), path), &LaunchCancellation::default())
        .expect("saved native page");
    assert_eq!(page.rows.len(), 32);
    assert_eq!(page.rows[0].id, "native-99");
    assert_eq!(page.rows[31].id, "native-68");
    assert!(!page.has_newer && page.has_older);
    let methods = server.join().expect("native peer");
    assert_eq!(
        methods,
        ["initialize", "initialized", "thread/list", "thread/list"]
    );
}

#[test]
fn cancelled_native_page_closes_the_captured_reader_without_reissuing_list() {
    let root = tempfile::tempdir().expect("private catalogue");
    let path = root.path().join("native.sock");
    let listener = UnixListener::bind(&path).expect("native socket");
    let (held, received) = mpsc::channel();
    let server = thread::spawn(move || serve(listener, Some(held)));
    let cancel = LaunchCancellation::default();
    let worker_cancel = cancel.clone();
    let request = request(root.path(), path);
    let worker = thread::spawn(move || read_page(request, &worker_cancel));
    received
        .recv_timeout(Duration::from_secs(5))
        .expect("list held");
    cancel.cancel();
    assert!(worker.join().expect("reader").is_err());
    assert_eq!(
        server.join().expect("native peer"),
        ["initialize", "initialized", "thread/list"]
    );
}

fn request(cwd: &std::path::Path, socket: std::path::PathBuf) -> SavedPageRequest {
    SavedPageRequest {
        backend: Backend::Codex,
        cwd: cwd.to_owned(),
        codex_socket: Some(socket),
        direction: PageDirection::Newest,
        allow_grok_fallback: false,
    }
}

fn serve(listener: UnixListener, held: Option<mpsc::Sender<()>>) -> Vec<String> {
    let stream = listener.accept().expect("accept native reader").0;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("deadline");
    let mut socket = tungstenite::accept(stream).expect("native handshake");
    let mut methods = Vec::new();
    let mut pages = 0;
    while let Ok(frame) = socket.read() {
        if !frame.is_text() {
            continue;
        }
        let request: Value = serde_json::from_str(frame.to_text().expect("text")).expect("request");
        let method = request["method"].as_str().expect("method");
        methods.push(method.to_owned());
        if method == "initialized" {
            continue;
        }
        let result = if method == "thread/list" {
            assert_eq!(request["params"]["sortKey"], "updated_at");
            assert_eq!(request["params"]["limit"], 64);
            if let Some(held) = &held {
                held.send(()).expect("notify held");
                continue;
            }
            assert_eq!(
                request["params"]["cursor"],
                if pages == 0 {
                    Value::Null
                } else {
                    json!("opaque-native-cursor")
                }
            );
            let range = if pages == 0 { 80..100 } else { 40..80 };
            pages += 1;
            json!({"data":range.rev().map(|index| json!({"id":format!("native-{index}"),"updatedAt":index})).collect::<Vec<_>>(),
                "nextCursor":"opaque-native-cursor"})
        } else {
            assert_eq!(method, "initialize");
            json!({})
        };
        socket
            .send(Message::Text(
                json!({"id":request["id"],"result":result})
                    .to_string()
                    .into(),
            ))
            .expect("native reply");
    }
    methods
}
