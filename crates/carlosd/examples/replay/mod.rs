//! Transfer identical typed pages through four queued frames plus one active socket write.

mod produce;
mod websocket;

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

use anyhow::Result;
use carlos_core::session::state::SessionState;
use carlos_proto::output::OutputChunk;
use carlos_proto::session::{History, Snapshot};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
enum Packet {
    Snapshot(Box<Snapshot>),
    History(History),
    Output(OutputChunk),
}

#[derive(Default)]
pub(super) struct Measured {
    pub rows: usize,
    pub outputs: usize,
    pub bytes: usize,
    pub frames: usize,
    pub encode_ms: f64,
    pub write_ms: f64,
    pub decode_ms: f64,
    pub wall_ms: f64,
}

pub(super) fn measure(session: SessionState, transport: &str) -> Result<Measured> {
    match transport {
        "unix" => unix(session),
        "websocket" => websocket::measure(session),
        _ => anyhow::bail!("transport must be unix or websocket"),
    }
}

fn unix(session: SessionState) -> Result<Measured> {
    let (mut writer, reader) = UnixStream::pair()?;
    socket2::SockRef::from(&writer).set_send_buffer_size(64 * 1024)?;
    let (frames, incoming) = mpsc::sync_channel::<Vec<u8>>(4);
    let start = Instant::now();
    let producer = thread::spawn(move || produce::pages(session, frames));
    let socket = thread::spawn(move || -> Result<f64> {
        let mut elapsed = 0.0;
        while let Ok(frame) = incoming.recv() {
            let start = Instant::now();
            writer.write_all(&frame)?;
            elapsed += start.elapsed().as_secs_f64() * 1000.0;
        }
        Ok(elapsed)
    });
    let mut measured = consume(reader)?;
    measured.encode_ms = producer.join().expect("fixture owner")?;
    measured.write_ms = socket.join().expect("fixture socket writer")?;
    measured.wall_ms = start.elapsed().as_secs_f64() * 1000.0;
    Ok(measured)
}

fn consume(reader: UnixStream) -> Result<Measured> {
    let mut measured = Measured::default();
    let mut reader = BufReader::new(reader);
    let mut line = Vec::new();
    while reader.read_until(b'\n', &mut line)? != 0 {
        measured.accept(&line)?;
        line.clear();
    }
    Ok(measured)
}

impl Measured {
    fn accept(&mut self, bytes: &[u8]) -> Result<()> {
        let start = Instant::now();
        let packet: Packet = serde_json::from_slice(bytes)?;
        self.decode_ms += start.elapsed().as_secs_f64() * 1000.0;
        match packet {
            Packet::Snapshot(snapshot) => self.rows += snapshot.entries.len(),
            Packet::History(history) => self.rows += history.entries.len(),
            Packet::Output(_) => self.outputs += 1,
        }
        self.bytes += bytes.len();
        self.frames += 1;
        Ok(())
    }
}
