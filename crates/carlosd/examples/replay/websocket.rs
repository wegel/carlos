//! Measure the shared page producer over real loopback WebSocket text messages.

use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use carlos_core::session::state::SessionState;
use tungstenite::client::client_with_config;
use tungstenite::protocol::WebSocketConfig;
use tungstenite::{accept_with_config, Message};

use super::{produce, Measured};

pub(super) fn measure(session: SessionState) -> Result<Measured> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    let (frames, incoming) = mpsc::sync_channel::<Vec<u8>>(4);
    let writer = thread::spawn(move || -> Result<f64> {
        let (stream, _) = listener.accept()?;
        configure(&stream)?;
        let mut socket = accept_with_config(stream, Some(buffers()))?;
        let mut elapsed = 0.0;
        while let Ok(frame) = incoming.recv() {
            let start = Instant::now();
            socket.send(Message::Text(String::from_utf8(frame)?.into()))?;
            elapsed += start.elapsed().as_secs_f64() * 1000.0;
        }
        socket.close(None)?;
        Ok(elapsed)
    });
    let stream = TcpStream::connect(address)?;
    configure(&stream)?;
    let (mut socket, _) = client_with_config(format!("ws://{address}"), stream, Some(buffers()))?;
    let start = Instant::now();
    let producer = thread::spawn(move || produce::pages(session, frames));
    let mut measured = Measured::default();
    loop {
        match socket.read()? {
            Message::Text(text) => measured.accept(text.as_bytes())?,
            Message::Close(_) => break,
            _ => anyhow::bail!("unexpected fixture message"),
        }
    }
    measured.encode_ms = producer.join().expect("fixture owner")?;
    measured.write_ms = writer.join().expect("fixture socket writer")?;
    measured.wall_ms = start.elapsed().as_secs_f64() * 1000.0;
    Ok(measured)
}

fn configure(stream: &TcpStream) -> Result<()> {
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    stream.set_write_timeout(Some(Duration::from_secs(30)))?;
    socket2::SockRef::from(stream).set_send_buffer_size(64 * 1024)?;
    socket2::SockRef::from(stream).set_recv_buffer_size(64 * 1024)?;
    Ok(())
}

fn buffers() -> WebSocketConfig {
    WebSocketConfig::default()
        .read_buffer_size(16384)
        .write_buffer_size(16384)
        .max_write_buffer_size(carlosd::limits::FRAME_BYTES + 32 * 1024)
        .max_message_size(Some(carlosd::limits::FRAME_BYTES))
        .max_frame_size(Some(carlosd::limits::FRAME_BYTES))
}
