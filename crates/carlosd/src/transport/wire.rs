//! Bound JSON envelopes before dispatch and before allocating a complete outgoing frame.

use std::io::{self, Write};
use std::sync::Arc;

use carlos_proto::commands::{Failure, Request, RequestId, RpcError, RpcVersion, Success};
use carlos_proto::events::{Notification, NotificationMessage};
use serde::Serialize;
use serde_json::Value;

pub(crate) fn decode(bytes: &[u8]) -> Result<Request, Failure> {
    let value: Value =
        serde_json::from_slice(bytes).map_err(|_| failure(None, -32700, "invalid JSON"))?;
    let id = value
        .get("id")
        .cloned()
        .and_then(|id| serde_json::from_value::<RequestId>(id).ok())
        .filter(|id| !matches!(id, RequestId::String(text) if text.len() > 256));
    if id.is_none() || value.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err(failure(
            id,
            -32600,
            "expected JSON-RPC 2.0 with a bounded request id",
        ));
    }
    let method = value.get("method").and_then(Value::as_str).unwrap_or("");
    let known = matches!(
        method,
        "hello"
            | "sessions/watch"
            | "session/create"
            | "session/subscribe"
            | "session/unsubscribe"
            | "session/history"
            | "session/send"
            | "session/rename"
            | "session/rewind"
            | "session/interrupt"
            | "session/approve"
            | "session/setRalph"
            | "goal/read"
            | "goal/set"
            | "goal/clear"
            | "task/run"
            | "task/stop"
            | "task/list"
            | "task/detail"
            | "task/text"
            | "queue/read"
            | "queue/text"
            | "queue/change"
            | "operation/read"
            | "entry/output"
            | "entry/output/subscribe"
            | "entry/output/unsubscribe"
            | "entry/source"
            | "entry/source/seek"
            | "entry/source/subscribe"
            | "entry/source/unsubscribe"
    );
    if !known {
        return Err(failure(id, -32601, "unknown Carlos method"));
    }
    serde_json::from_value(value).map_err(|_| failure(id, -32602, "invalid command fields"))
}

pub(crate) fn encode(value: &impl Serialize) -> io::Result<Arc<[u8]>> {
    let mut output = Limited(Vec::new());
    serde_json::to_writer(&mut output, value).map_err(io::Error::other)?;
    output.write_all(b"\n")?;
    Ok(output.0.into())
}

pub(crate) fn success(id: RequestId, result: impl Serialize) -> io::Result<Arc<[u8]>> {
    encode(&Success {
        jsonrpc: RpcVersion::V2,
        id,
        result,
    })
}

pub(crate) fn notification(notification: Notification) -> io::Result<Arc<[u8]>> {
    encode(&NotificationMessage {
        jsonrpc: RpcVersion::V2,
        notification,
    })
}

pub(crate) fn failure(id: Option<RequestId>, code: i32, message: &str) -> Failure {
    Failure {
        jsonrpc: RpcVersion::V2,
        id,
        error: error(code, message),
    }
}

pub(crate) fn error(code: i32, message: impl Into<String>) -> RpcError {
    RpcError {
        code,
        message: message.into(),
    }
}

struct Limited(Vec<u8>);
impl Write for Limited {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > crate::limits::FRAME_BYTES.saturating_sub(self.0.len()) {
            return Err(io::Error::other("encoded frame exceeds eight MiB"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
