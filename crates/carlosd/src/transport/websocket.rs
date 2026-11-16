//! Own each WebSocket on one thread and charge pending bytes until its real flush completes.

#[path = "websocket_poll.rs"]
mod polling;

use std::io;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

use carlos_proto::commands::Command;
use tungstenite::{Bytes, Error, Message, Utf8Bytes};

use super::ingress::Ingress;
use super::outbox::{Frame, Kind, Outbox};
use super::{wire, CloseHandle, PeerSocket};
use crate::network::{Accepted, Admission, Socket};

struct Controls {
    input: Ingress,
    output: Outbox,
    closed: Arc<AtomicBool>,
    manager: mpsc::SyncSender<()>,
}

#[derive(Default)]
struct Pending {
    frame: Option<Frame>,
    deadline: Option<Instant>,
    closing: bool,
}

struct Worker {
    id: u64,
    socket: Socket,
    admission: Arc<Admission>,
    controls: Controls,
    wake: UnixStream,
    pending: Pending,
    greeted: bool,
}

pub(super) fn start(
    id: u64,
    accepted: Accepted,
    input: Ingress,
    manager: mpsc::SyncSender<()>,
) -> io::Result<PeerSocket> {
    let stream = accepted.socket.get_ref().stream.try_clone()?;
    let (wake, notify) = UnixStream::pair()?;
    wake.set_nonblocking(true)?;
    let output = Outbox::with_wake(notify)?;
    let closed = Arc::new(AtomicBool::new(false));
    let admission = accepted.admission.clone();
    let worker = Worker {
        id,
        socket: accepted.socket,
        admission: accepted.admission,
        controls: Controls {
            input,
            output: output.clone(),
            closed: closed.clone(),
            manager,
        },
        wake,
        pending: Pending::default(),
        greeted: false,
    };
    let thread = thread::spawn(move || {
        let _ = worker.run();
    });
    Ok(PeerSocket {
        outbox: output,
        closed,
        stream: CloseHandle::Tcp(stream),
        threads: vec![thread],
        network: Some(admission),
    })
}

impl Worker {
    fn run(mut self) -> io::Result<()> {
        let mut authenticated = false;
        loop {
            if self.controls.closed.load(Ordering::Acquire) {
                return Ok(());
            }
            authenticated = self.authenticate(authenticated);
            if !authenticated && Instant::now() >= self.admission.deadline {
                return Err(io::ErrorKind::TimedOut.into());
            }
            let mut progressed = self.write()?;
            authenticated = self.authenticate(authenticated);
            if self.pending.closing && self.pending.deadline.is_none() {
                return Ok(());
            }
            if !self.pending.closing {
                progressed |= self.read(authenticated)?;
            }
            progressed |= self.write()?;
            if progressed {
                continue;
            }
            let deadline = self
                .pending
                .deadline
                .into_iter()
                .chain((!authenticated).then_some(self.admission.deadline))
                .min();
            polling::wait(
                &self.socket.get_ref().stream,
                &self.wake,
                !self.pending.closing,
                self.pending.deadline.is_some(),
                deadline,
            )?;
        }
    }

    fn authenticate(&mut self, authenticated: bool) -> bool {
        if authenticated || !self.admission.is_authenticated() {
            return authenticated;
        }
        self.socket.set_config(|config| {
            config.max_message_size = Some(crate::limits::REQUEST_BYTES);
            config.max_frame_size = Some(crate::limits::REQUEST_BYTES);
        });
        true
    }

    fn read(&mut self, authenticated: bool) -> io::Result<bool> {
        let mut progressed = false;
        for _ in 0..32 {
            match self.socket.read() {
                Ok(Message::Text(text)) => {
                    self.request(text.as_bytes(), authenticated)?;
                    progressed = true;
                    if !authenticated {
                        break;
                    }
                }
                Ok(Message::Ping(_)) => {
                    self.flushing();
                    progressed = true;
                }
                Ok(Message::Pong(_)) => progressed = true,
                Ok(Message::Close(_)) => {
                    self.pending.closing = true;
                    self.flushing();
                    break;
                }
                Ok(_) => return Err(io::ErrorKind::InvalidData.into()),
                Err(error) if blocked(&error) => break,
                Err(_) => return Err(io::ErrorKind::ConnectionAborted.into()),
            }
        }
        Ok(progressed)
    }

    fn request(&mut self, bytes: &[u8], authenticated: bool) -> io::Result<()> {
        let request = match wire::decode(bytes) {
            Ok(request) => request,
            Err(error) if authenticated => {
                let bytes = wire::encode(&error)?;
                return self
                    .controls
                    .output
                    .push(Frame {
                        bytes,
                        kind: Kind::Reply,
                    })
                    .then_some(())
                    .ok_or_else(|| io::ErrorKind::OutOfMemory.into());
            }
            Err(_) => return Err(io::ErrorKind::InvalidData.into()),
        };
        if !authenticated && (self.greeted || !matches!(request.command, Command::Hello(_))) {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        if !self
            .controls
            .input
            .push_network(self.id, request, bytes.len())
        {
            return Err(io::ErrorKind::OutOfMemory.into());
        }
        self.greeted = true;
        Ok(())
    }

    fn flushing(&mut self) {
        self.pending
            .deadline
            .get_or_insert_with(|| Instant::now() + Duration::from_millis(500));
    }

    fn write(&mut self) -> io::Result<bool> {
        if self.pending.deadline.is_none() && !self.pending.closing {
            if let Some(frame) = self.controls.output.try_take() {
                let text = Utf8Bytes::try_from(Bytes::from_owner(frame.bytes.clone()))
                    .map_err(|_| io::ErrorKind::InvalidData)?;
                self.pending.frame = Some(frame);
                self.flushing();
                if let Err(error) = self.socket.write(Message::Text(text)) {
                    if !blocked(&error) {
                        return Err(io::ErrorKind::ConnectionAborted.into());
                    }
                }
            }
        }
        let Some(deadline) = self.pending.deadline else {
            return Ok(false);
        };
        if Instant::now() >= deadline {
            return Err(io::ErrorKind::TimedOut.into());
        }
        match self.socket.flush() {
            Ok(()) => {
                if let Some(frame) = self.pending.frame.take() {
                    self.controls.output.finish(&frame);
                }
                self.pending.deadline = None;
                Ok(true)
            }
            Err(error) if blocked(&error) => Ok(false),
            Err(Error::ConnectionClosed) if self.pending.closing => {
                self.pending.deadline = None;
                Ok(true)
            }
            Err(_) => Err(io::ErrorKind::ConnectionAborted.into()),
        }
    }
}

fn blocked(error: &Error) -> bool {
    matches!(error, Error::Io(error) if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted))
}

impl Drop for Worker {
    fn drop(&mut self) {
        if let Some(frame) = self.pending.frame.take() {
            self.controls.output.finish(&frame);
        }
        self.controls.closed.store(true, Ordering::Release);
        self.controls.output.close();
        let _ = self.socket.get_ref().stream.shutdown(Shutdown::Both);
        let _ = self.controls.manager.try_send(());
    }
}
