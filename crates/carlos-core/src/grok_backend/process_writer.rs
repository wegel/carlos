//! Bound ACP input bytes and wait time without locking the Grok event owner.

use std::io::Write;
use std::process::Child;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::dispatch_guard::{DispatchGuard, DispatchRetired};
use anyhow::{bail, ensure, Result};
use serde_json::Value;

const MAX_FRAME: usize = 8 * 1024 * 1024;
const MAX_QUEUED: usize = 16 * 1024 * 1024;
const WRITE_TIMEOUT: Duration = Duration::from_millis(500);
type Job = (String, DispatchGuard, mpsc::Sender<Result<()>>);

pub(super) struct InputWriter {
    sender: mpsc::SyncSender<Job>,
    queued: Arc<AtomicUsize>,
    closed: Arc<AtomicBool>,
    child: Arc<Mutex<Child>>,
}

impl InputWriter {
    pub(super) fn start(mut stdin: impl Write + Send + 'static, child: Arc<Mutex<Child>>) -> Self {
        let (sender, receiver) = mpsc::sync_channel::<Job>(8);
        let queued = Arc::new(AtomicUsize::new(0));
        let closed = Arc::new(AtomicBool::new(false));
        let bytes = queued.clone();
        let stopped = closed.clone();
        thread::spawn(move || {
            while let Ok((line, permission, reply)) = receiver.recv() {
                let result = if let Err(error) = permission.check() {
                    Err(anyhow::Error::new(error))
                } else if stopped.load(Ordering::SeqCst) {
                    Err(anyhow::anyhow!("Grok stdin is closed"))
                } else {
                    stdin
                        .write_all(line.as_bytes())
                        .and_then(|()| stdin.flush())
                        .map_err(anyhow::Error::from)
                };
                bytes.fetch_sub(line.len(), Ordering::SeqCst);
                let _ = reply.send(result);
            }
        });
        Self {
            sender,
            queued,
            closed,
            child,
        }
    }

    pub(super) fn write(&self, frame: &Value) -> Result<()> {
        self.write_guarded(frame, DispatchGuard::default())
    }

    pub(super) fn write_guarded(&self, frame: &Value, permission: DispatchGuard) -> Result<()> {
        let receiver = self.enqueue(frame, permission)?;
        match receiver.recv_timeout(WRITE_TIMEOUT) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) if error.is::<DispatchRetired>() => Err(error),
            other => {
                self.close();
                match other {
                    Ok(Err(error)) => bail!("write Grok stdin: {error}"),
                    _ => bail!("Grok stdin did not finish within 500 ms; request may already have been accepted"),
                }
            }
        }
    }

    pub(super) fn enqueue(
        &self,
        frame: &Value,
        permission: DispatchGuard,
    ) -> Result<mpsc::Receiver<Result<()>>> {
        permission.check()?;
        ensure!(!self.closed.load(Ordering::SeqCst), "Grok stdin is closed");
        let line = format!("{frame}\n");
        ensure!(line.len() <= MAX_FRAME, "Grok request exceeds 8 MiB");
        let length = line.len();
        if self
            .queued
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |bytes| {
                bytes.checked_add(length).filter(|sum| *sum <= MAX_QUEUED)
            })
            .is_err()
        {
            self.close();
            bail!("Grok stdin backlog exceeds 16 MiB");
        }
        let (reply, receiver) = mpsc::channel();
        if self.sender.try_send((line, permission, reply)).is_err() {
            self.queued.fetch_sub(length, Ordering::SeqCst);
            self.close();
            bail!("Grok stdin queue is full or closed");
        }
        Ok(receiver)
    }

    pub(super) fn close(&self) {
        if !self.closed.swap(true, Ordering::SeqCst) {
            let _ = self.child.lock().expect("Grok child").kill();
        }
    }
}

#[cfg(test)]
#[path = "writer_guard_tests.rs"]
mod guard_tests;
