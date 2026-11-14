//! Move bounded Unix frames without making the manager wait for a device socket.

pub(crate) mod ingress;
pub(crate) mod outbox;
mod reader;
pub(crate) mod wire;

use std::io::{self, Write};
use std::net::Shutdown;
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use self::ingress::Ingress;
use self::outbox::Outbox;

pub(crate) struct PeerSocket {
    pub outbox: Outbox,
    pub closed: Arc<AtomicBool>,
    stream: UnixStream,
    threads: Vec<JoinHandle<()>>,
}

impl PeerSocket {
    pub(crate) fn start(
        id: u64,
        stream: UnixStream,
        ingress: Ingress,
        wake: mpsc::SyncSender<()>,
    ) -> io::Result<Self> {
        stream.set_nonblocking(true)?;
        socket2::SockRef::from(&stream).set_send_buffer_size(64 * 1024)?;
        socket2::SockRef::from(&stream).set_recv_buffer_size(64 * 1024)?;
        let read = stream.try_clone()?;
        let write = stream.try_clone()?;
        let outbox = Outbox::default();
        let closed = Arc::new(AtomicBool::new(false));
        let reader_box = outbox.clone();
        let reader_closed = closed.clone();
        let reader_wake = wake.clone();
        let reader = thread::spawn(move || {
            reader::run(id, &read, ingress, &reader_box);
            close(&read, &reader_box, &reader_closed, &reader_wake);
        });
        let writer_box = outbox.clone();
        let writer_closed = closed.clone();
        let writer = thread::spawn(move || {
            while let Some(frame) = writer_box.take() {
                let result = write_frame(&write, &frame.bytes);
                writer_box.finish(&frame);
                if result.is_err() {
                    break;
                }
            }
            close(&write, &writer_box, &writer_closed, &wake);
        });
        Ok(Self {
            outbox,
            closed,
            stream,
            threads: vec![reader, writer],
        })
    }

    pub(crate) fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        self.outbox.close();
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}

impl Drop for PeerSocket {
    fn drop(&mut self) {
        self.close();
        for worker in self.threads.drain(..) {
            let _ = worker.join();
        }
    }
}

fn close(stream: &UnixStream, outbox: &Outbox, closed: &AtomicBool, wake: &mpsc::SyncSender<()>) {
    closed.store(true, Ordering::SeqCst);
    outbox.close();
    let _ = stream.shutdown(Shutdown::Both);
    let _ = wake.try_send(());
}

pub(crate) fn ready(stream: &UnixStream, events: i16, timeout: Duration) -> io::Result<bool> {
    let mut fd = libc::pollfd {
        fd: stream.as_raw_fd(),
        events,
        revents: 0,
    };
    let result =
        unsafe { libc::poll(&mut fd, 1, timeout.as_millis().min(i32::MAX as u128) as i32) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(result > 0)
}

fn write_frame(mut stream: &UnixStream, mut bytes: &[u8]) -> io::Result<()> {
    let deadline = Instant::now() + Duration::from_millis(500);
    while !bytes.is_empty() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::ErrorKind::TimedOut.into());
        }
        match stream.write(bytes) {
            Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
            Ok(count) => bytes = &bytes[count..],
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                ready(stream, libc::POLLOUT, remaining)?;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
