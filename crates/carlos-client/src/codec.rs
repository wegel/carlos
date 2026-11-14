//! Bound complete JSON messages and incremental Unix framing without owning a socket or thread.

use std::io::{self, Write};

use carlos_proto::commands::{Request, RequestId, RpcError, RpcVersion};
use carlos_proto::events::{Notification, NotificationMessage};
use serde_json::Value;

use crate::ClientError;

/// Both stream and WebSocket platforms check this limit before parsing an entire message.
pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

/// The pending request determines the type of a successful result.
#[derive(Debug)]
pub enum Incoming {
    Notification(Box<Notification>),
    Success {
        id: RequestId,
        result: Value,
    },
    Failure {
        id: Option<RequestId>,
        error: RpcError,
    },
}

/// Encode one typed Carlos command, including its Unix-stream delimiter.
pub fn encode(request: &Request) -> Result<Vec<u8>, ClientError> {
    if matches!(&request.id, RequestId::String(id) if id.len() > 256) {
        return Err(ClientError::InvalidEnvelope);
    }
    let mut writer = RequestBytes(Vec::new());
    serde_json::to_writer(&mut writer, request).map_err(|error| {
        if error.is_io() {
            ClientError::Capacity("client request exceeds four MiB")
        } else {
            payload_error(error)
        }
    })?;
    writer.0.push(b'\n');
    Ok(writer.0)
}

struct RequestBytes(Vec<u8>);

impl Write for RequestBytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() >= (4 * 1024 * 1024usize).saturating_sub(self.0.len()) {
            return Err(io::Error::other("request byte limit"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Decode one complete server message; never accept an ambiguous result/error object.
pub fn decode(bytes: &[u8]) -> Result<Incoming, ClientError> {
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(ClientError::FrameTooLarge);
    }
    let mut value: Value = serde_json::from_slice(bytes).map_err(payload_error)?;
    if value["jsonrpc"] != "2.0" {
        return Err(ClientError::InvalidEnvelope);
    }
    if value.get("method").is_some() {
        if value.get("id").is_some()
            || value.get("result").is_some()
            || value.get("error").is_some()
        {
            return Err(ClientError::InvalidEnvelope);
        }
        return serde_json::from_value::<NotificationMessage>(value)
            .map(|message| Incoming::Notification(Box::new(message.notification)))
            .map_err(payload_error);
    }
    let object = value.as_object_mut().ok_or(ClientError::InvalidEnvelope)?;
    if object.contains_key("result") == object.contains_key("error") {
        return Err(ClientError::InvalidEnvelope);
    }
    let id = object.remove("id").ok_or(ClientError::InvalidEnvelope)?;
    if id.as_str().is_some_and(|text| text.len() > 256) {
        return Err(ClientError::InvalidEnvelope);
    }
    if let Some(error) = object.remove("error") {
        return Ok(Incoming::Failure {
            id: serde_json::from_value(id).map_err(payload_error)?,
            error: serde_json::from_value(error).map_err(payload_error)?,
        });
    }
    Ok(Incoming::Success {
        id: serde_json::from_value(id).map_err(payload_error)?,
        result: object.remove("result").expect("checked result"),
    })
}

/// Retain at most one bounded stream buffer and scan only newly arrived bytes for delimiters.
#[derive(Default)]
pub struct Framer {
    bytes: Vec<u8>,
    start: usize,
    scan: usize,
}

impl Framer {
    /// Append platform bytes after draining complete messages; failure preserves the old buffer.
    pub fn push(&mut self, bytes: &[u8]) -> Result<(), ClientError> {
        if bytes.len() > MAX_FRAME_BYTES.saturating_sub(self.bytes.len() - self.start) {
            return Err(ClientError::FrameTooLarge);
        }
        if self.start > 0
            && (self.start >= self.bytes.len() / 2
                || self.bytes.len() + bytes.len() > MAX_FRAME_BYTES)
        {
            self.bytes.drain(..self.start);
            self.scan -= self.start;
            self.start = 0;
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    /// Decode at most one complete line; a partial UTF-8 code point remains untouched until complete.
    pub fn next_message(&mut self) -> Result<Option<Incoming>, ClientError> {
        let Some(relative) = self.bytes[self.scan..]
            .iter()
            .position(|byte| *byte == b'\n')
        else {
            self.scan = self.bytes.len();
            return Ok(None);
        };
        let end = self.scan + relative;
        let result = decode(&self.bytes[self.start..end]);
        self.start = end + 1;
        self.scan = self.start;
        result.map(Some)
    }

    /// After draining complete messages, report whether EOF cut the final frame short.
    pub fn has_partial(&self) -> bool {
        self.start < self.bytes.len()
    }
}

fn payload_error(error: serde_json::Error) -> ClientError {
    ClientError::InvalidPayload(error.to_string().chars().take(256).collect())
}

/// Build requests without allowing platforms to change the RPC version.
pub fn request(id: RequestId, command: carlos_proto::commands::Command) -> Request {
    Request {
        jsonrpc: RpcVersion::V2,
        id,
        command,
    }
}
