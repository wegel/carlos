//! Keep a single bounded WebSocket codec over a local Unix stream, with independent shutdown.

use std::io::{self, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use mio::unix::SourceFd;
use mio::{Events, Interest, Poll, Token, Waker};
use socket2::{Domain, SockAddr, Socket, Type};
use tungstenite::client::client_with_config;
use tungstenite::protocol::WebSocketConfig;
use tungstenite::{Error, Message, WebSocket};

use super::{PendingReader, SocketParts};
use crate::protocol_events::EventQueue;
use crate::protocol_state::ClientState;
use crate::protocol_writer::MAX_FRAME_BYTES;

type Job = (String, mpsc::Sender<std::result::Result<(), String>>);

/// Own exactly one codec so fragments, automatic pong and close frames share protocol state.
pub(crate) struct SocketReader {
    socket: UnixStream,
    incoming: mpsc::Receiver<Job>,
    poll: Poll,
}

struct SocketInput {
    outgoing: mpsc::SyncSender<Job>,
    state: Arc<ClientState>,
    wake: Arc<Waker>,
}

/// Bound local connect and retain a shutdown handle before starting the codec.
pub(super) fn prepare(path: &Path, events: EventQueue) -> Result<SocketParts> {
    let socket = Socket::new(Domain::UNIX, Type::STREAM, None)?;
    socket
        .connect_timeout(&SockAddr::unix(path)?, Duration::from_secs(2))
        .with_context(|| format!("Connect Codex socket {}", path.display()))?;
    let descriptor: std::os::fd::OwnedFd = socket.into();
    let socket = UnixStream::from(descriptor);
    let state = ClientState::new_socket(socket.try_clone()?, events);
    let poll = Poll::new()?;
    poll.registry().register(
        &mut SourceFd(&socket.as_raw_fd()),
        Token(0),
        Interest::READABLE | Interest::WRITABLE,
    )?;
    let wake = Arc::new(Waker::new(poll.registry(), Token(1))?);
    let (outgoing, incoming) = mpsc::sync_channel(1);
    Ok(SocketParts {
        state: state.clone(),
        input: Box::new(SocketInput {
            outgoing,
            state,
            wake,
        }),
        reader: PendingReader::Socket(SocketReader {
            socket,
            incoming,
            poll,
        }),
    })
}

impl SocketReader {
    /// Run one codec only after the manager can answer this connection’s callbacks.
    pub(super) fn start(self, state: Arc<ClientState>) {
        thread::spawn(move || {
            let result = self.run(&state);
            let message = result.err().map_or_else(
                || "Codex socket connection closed".into(),
                |error| format!("Codex socket: {error:#}"),
            );
            state.close(&message, true);
        });
    }

    fn run(self, state: &Arc<ClientState>) -> Result<()> {
        self.socket.set_read_timeout(Some(Duration::from_secs(2)))?;
        self.socket
            .set_write_timeout(Some(Duration::from_secs(2)))?;
        let (finished, ready) = mpsc::channel();
        let watched = state.clone();
        thread::spawn(move || {
            if matches!(
                ready.recv_timeout(Duration::from_secs(2)),
                Err(mpsc::RecvTimeoutError::Timeout)
            ) {
                watched.close("Codex local WebSocket handshake exceeded2 seconds", true);
            }
        });
        let config = WebSocketConfig::default()
            .read_buffer_size(16 * 1024)
            .write_buffer_size(0)
            .max_write_buffer_size(MAX_FRAME_BYTES + 1024)
            .max_frame_size(Some(crate::protocol_reader::MAX_HISTORY_REPLY_BYTES))
            .max_message_size(Some(crate::protocol_reader::MAX_HISTORY_REPLY_BYTES));
        // Official Unix clients use this URI; these bytes never open a TCP connection.
        let result = client_with_config("ws://localhost/rpc", self.socket, Some(config));
        let _ = finished.send(());
        let (mut socket, response) =
            result.map_err(|error| anyhow::anyhow!("Local WebSocket handshake: {error}"))?;
        anyhow::ensure!(
            !response
                .headers()
                .contains_key("x-codex-websocket-compression"),
            "Native selected unrequested compression"
        );
        socket.get_mut().set_read_timeout(None)?;
        socket.get_mut().set_write_timeout(None)?;
        socket.get_mut().set_nonblocking(true)?;
        relay(&mut socket, self.incoming, self.poll, state)
    }
}

fn relay(
    socket: &mut WebSocket<UnixStream>,
    incoming: mpsc::Receiver<Job>,
    mut poll: Poll,
    state: &ClientState,
) -> Result<()> {
    let mut pending: Option<Job> = None;
    let mut events = Events::with_capacity(8);
    while !state.is_closed() {
        if pending.is_none() {
            match incoming.try_recv() {
                Ok(job) => {
                    let body = job.0.trim_end_matches('\n').to_string();
                    match socket.write(Message::Text(body.into())) {
                        Ok(()) => {}
                        Err(error) if blocked(&error) => {}
                        Err(error) => {
                            let _ = job.1.send(Err(error.to_string()));
                            return Err(error.into());
                        }
                    }
                    pending = Some(job);
                }
                Err(mpsc::TryRecvError::Disconnected) => break,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        match socket.flush() {
            Ok(()) => {
                if let Some(job) = pending.take() {
                    let _ = job.1.send(Ok(()));
                }
            }
            Err(error) if blocked(&error) => {}
            Err(error) => return Err(error.into()),
        }
        if !read_available(socket, state)? && !state.is_closed() {
            poll.poll(&mut events, None)?;
        }
    }
    Ok(())
}

fn read_available(socket: &mut WebSocket<UnixStream>, state: &ClientState) -> Result<bool> {
    for _ in 0..64 {
        match socket.read() {
            Ok(Message::Text(text)) => crate::protocol_reader::route(&text, state)?,
            Ok(Message::Ping(_) | Message::Pong(_)) => {}
            Ok(Message::Close(frame)) => bail!("Native closed the local connection: {frame:?}"),
            Ok(_) => bail!("Native local connection sent unrequested binary data"),
            Err(error) if blocked(&error) => return Ok(false),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(true)
}

fn blocked(error: &Error) -> bool {
    matches!(error, Error::Io(error) if error.kind() == io::ErrorKind::WouldBlock)
}

impl Write for SocketInput {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let line = std::str::from_utf8(buffer)
            .map_err(io::Error::other)?
            .to_string();
        let (reply, receiver) = mpsc::channel();
        self.outgoing
            .try_send((line, reply))
            .map_err(io::Error::other)?;
        self.wake.wake().map_err(io::Error::other)?;
        while !self.state.is_closed() {
            match receiver.recv_timeout(Duration::from_millis(25)) {
                Ok(result) => return result.map(|()| buffer.len()).map_err(io::Error::other),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(error) => return Err(io::Error::other(error)),
            }
        }
        Err(io::Error::other("Codex socket is closed"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
