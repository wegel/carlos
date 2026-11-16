//! Cap upgrade bytes and total login time before a WebSocket can reach the session manager.

use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::time::Instant;

use tungstenite::handshake::server::{Callback, ErrorResponse, Request, Response};
use tungstenite::http::header::{ORIGIN, SEC_WEBSOCKET_PROTOCOL};
use tungstenite::protocol::WebSocketConfig;
use tungstenite::{accept_hdr_with_config, WebSocket};

pub(crate) type Socket = WebSocket<HandshakeStream>;

pub(crate) struct HandshakeStream {
    pub stream: TcpStream,
    pub written: u64,
    deadline: Instant,
    received: usize,
    handshaking: bool,
}

struct UpgradePolicy {
    origin: Option<String>,
}

impl Callback for UpgradePolicy {
    fn on_request(self, request: &Request, response: Response) -> Result<Response, ErrorResponse> {
        if allowed(request, self.origin.as_deref()) {
            Ok(response)
        } else {
            Err(refused())
        }
    }
}

pub(crate) fn upgrade(
    stream: TcpStream,
    origin: Option<String>,
    deadline: Instant,
) -> io::Result<Socket> {
    stream.set_nonblocking(false)?;
    stream.set_nodelay(true)?;
    socket2::SockRef::from(&stream).set_send_buffer_size(64 * 1024)?;
    socket2::SockRef::from(&stream).set_recv_buffer_size(64 * 1024)?;
    let stream = HandshakeStream {
        stream,
        written: 0,
        deadline,
        received: 0,
        handshaking: true,
    };
    let config = WebSocketConfig::default()
        .read_buffer_size(16384)
        .write_buffer_size(16384)
        .max_write_buffer_size(crate::limits::FRAME_BYTES + 32 * 1024)
        .max_message_size(Some(4096))
        .max_frame_size(Some(4096))
        .accept_unmasked_frames(false);
    let mut socket = accept_hdr_with_config(stream, UpgradePolicy { origin }, Some(config))
        .map_err(|_| io::Error::other("WebSocket upgrade refused"))?;
    let stream = socket.get_mut();
    stream.handshaking = false;
    stream.stream.set_read_timeout(None)?;
    stream.stream.set_write_timeout(None)?;
    stream.stream.set_nonblocking(true)?;
    Ok(socket)
}

fn allowed(request: &Request, allowed_origin: Option<&str>) -> bool {
    if request.uri().path() != "/"
        || request.uri().query().is_some()
        || request.uri().scheme().is_some()
        || request.uri().authority().is_some()
        || request.headers().contains_key(SEC_WEBSOCKET_PROTOCOL)
    {
        return false;
    }
    let mut origins = request.headers().get_all(ORIGIN).iter();
    let origin = origins.next();
    if origins.next().is_some() {
        return false;
    }
    match origin {
        None => true,
        Some(origin) => origin
            .to_str()
            .ok()
            .zip(allowed_origin)
            .is_some_and(|(actual, allowed)| actual == allowed),
    }
}

fn refused() -> ErrorResponse {
    Response::builder()
        .status(403)
        .body(Some("WebSocket upgrade refused".into()))
        .expect("fixed refusal")
}

impl HandshakeStream {
    fn remaining(&self) -> io::Result<std::time::Duration> {
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::ErrorKind::TimedOut.into());
        }
        Ok(remaining)
    }
}

impl Read for HandshakeStream {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if !self.handshaking || buffer.is_empty() {
            return self.stream.read(buffer);
        }
        let limit = (8192usize).saturating_sub(self.received).min(buffer.len());
        if limit == 0 {
            return Err(io::ErrorKind::InvalidData.into());
        }
        self.stream.set_read_timeout(Some(self.remaining()?))?;
        let count = self.stream.read(&mut buffer[..limit])?;
        self.received += count;
        Ok(count)
    }
}

impl Write for HandshakeStream {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.handshaking {
            self.stream.set_write_timeout(Some(self.remaining()?))?;
        }
        let count = self.stream.write(bytes)?;
        self.written = self.written.wrapping_add(count as u64);
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}
