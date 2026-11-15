//! Bound nonblocking Unix reads and writes while the pure client owns protocol state.

use std::collections::VecDeque;
use std::io::{ErrorKind, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;

use anyhow::{bail, Context, Result};
use carlos_client::codec::{Framer, Incoming};
use carlos_client::{Client, ConnectionId, Outbound};

struct PendingWrite {
    request: Outbound,
    offset: usize,
}

pub(in crate::app) struct Transport {
    stream: UnixStream,
    pub connection: ConnectionId,
    framer: Framer,
    writes: VecDeque<PendingWrite>,
    bytes: usize,
}

impl Transport {
    pub fn connect(path: &Path, client: &mut Client) -> Result<Self> {
        let stream = UnixStream::connect(path).context("connect to Carlos daemon")?;
        Self::from_stream(stream, client)
    }

    pub fn from_stream(stream: UnixStream, client: &mut Client) -> Result<Self> {
        stream.set_nonblocking(true)?;
        let hello = client.begin("terminal".into())?;
        let mut link = Self {
            stream,
            connection: hello.connection(),
            framer: Framer::default(),
            writes: VecDeque::new(),
            bytes: 0,
        };
        link.send(hello)?;
        Ok(link)
    }

    pub fn send(&mut self, request: Outbound) -> Result<()> {
        if request.connection() != self.connection {
            bail!("outgoing request belongs to a retired socket");
        }
        if self.writes.len() >= 128
            || request.bytes().len() > (4 * 1024 * 1024usize).saturating_sub(self.bytes)
        {
            bail!("terminal socket writer is full");
        }
        self.bytes += request.bytes().len();
        self.writes.push_back(PendingWrite { request, offset: 0 });
        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        let mut remaining = 256 * 1024;
        while remaining > 0 {
            let Some(pending) = self.writes.front_mut() else {
                break;
            };
            let bytes = &pending.request.bytes()[pending.offset..];
            let size = bytes.len().min(remaining);
            match self.stream.write(&bytes[..size]) {
                Ok(0) => bail!("Carlos daemon closed the socket during a write"),
                Ok(written) => {
                    pending.offset += written;
                    remaining -= written;
                    if pending.offset == pending.request.bytes().len() {
                        self.bytes -= pending.offset;
                        self.writes.pop_front();
                    }
                }
                Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }

    pub fn receive(&mut self) -> Result<Vec<Incoming>> {
        let mut incoming = Vec::new();
        let mut bytes = [0; 16384];
        for _ in 0..16 {
            while incoming.len() < 64 {
                let Some(message) = self.framer.next_message()? else {
                    break;
                };
                incoming.push(message);
            }
            if incoming.len() >= 64 {
                break;
            }
            match self.stream.read(&mut bytes) {
                Ok(0) if incoming.is_empty() => bail!("Carlos daemon closed the connection"),
                Ok(0) => break,
                Ok(count) => self.framer.push(&bytes[..count])?,
                Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Ok(incoming)
    }
}
