//! Bound queued Codex stdin bytes and include pipe writes in request deadlines.

use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Instant;

use anyhow::{bail, ensure, Result};
use serde_json::Value;

use crate::dispatch_guard::{DispatchGuard, DispatchRetired};
use crate::protocol_state::ClientState;

/// Limit each stdin/stdout frame before it can fill an unbounded buffer.
pub(crate) const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;
const MAX_WRITE_BYTES: usize = 16 * 1024 * 1024;
type Job = (String, Instant, DispatchGuard, mpsc::Sender<Result<()>>);

/// Give stdin a single worker so the UI never waits on a blocking OS pipe.
pub(crate) struct InputWriter {
    sender: mpsc::SyncSender<Job>,
    bytes: Arc<AtomicUsize>,
}

impl InputWriter {
    /// Start a bounded worker that stops queued writes after terminal closure.
    pub(crate) fn start(mut stdin: Box<dyn Write + Send>, state: Arc<ClientState>) -> Self {
        let (sender, receiver) = mpsc::sync_channel::<Job>(8);
        let bytes = Arc::new(AtomicUsize::new(0));
        let queued = bytes.clone();
        thread::spawn(move || {
            while let Ok((line, deadline, permission, reply)) = receiver.recv() {
                let result = if let Err(error) = permission.check() {
                    Err(error.into())
                } else if state.is_closed() || Instant::now() >= deadline {
                    Err(anyhow::anyhow!(
                        "Codex closed or stdin deadline expired before dispatch"
                    ))
                } else {
                    stdin
                        .write_all(line.as_bytes())
                        .and_then(|_| stdin.flush())
                        .map_err(|error| anyhow::anyhow!("Write Codex stdin: {error}"))
                };
                queued.fetch_sub(line.len(), Ordering::SeqCst);
                if let Err(error) = &result {
                    if !error.is::<DispatchRetired>() {
                        state.close(&error.to_string(), true);
                    }
                }
                let _ = reply.send(result);
            }
        });
        Self { sender, bytes }
    }

    /// Reject oversized frames before enqueueing; bound both queueing and writing.
    pub(crate) fn write(&self, frame: Value, deadline: Instant, state: &ClientState) -> Result<()> {
        let receiver = self.enqueue(frame, deadline, state)?;
        wait(receiver, deadline, state)
    }

    pub(crate) fn write_guarded(
        &self,
        frame: Value,
        deadline: Instant,
        state: &ClientState,
        permission: DispatchGuard,
    ) -> Result<()> {
        wait(
            self.enqueue_guarded(frame, deadline, state, permission)?,
            deadline,
            state,
        )
    }

    /// Accept bounded async work without making the UI wait for the OS pipe.
    pub(crate) fn enqueue(
        &self,
        frame: Value,
        deadline: Instant,
        state: &ClientState,
    ) -> Result<mpsc::Receiver<Result<()>>> {
        self.enqueue_guarded(frame, deadline, state, DispatchGuard::default())
    }

    pub(crate) fn enqueue_guarded(
        &self,
        frame: Value,
        deadline: Instant,
        state: &ClientState,
        permission: DispatchGuard,
    ) -> Result<mpsc::Receiver<Result<()>>> {
        permission.check()?;
        let line = format!("{frame}\n");
        ensure!(line.len() <= MAX_FRAME_BYTES, "Codex request exceeds 8 MiB");
        ensure!(!state.is_closed(), "Codex app-server is closed");
        let length = line.len();
        let (reply, receiver) = mpsc::channel();
        if self
            .bytes
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |bytes| {
                bytes
                    .checked_add(length)
                    .filter(|sum| *sum <= MAX_WRITE_BYTES)
            })
            .is_err()
        {
            state.close("Codex stdin backlog exceeds 16 MiB", true);
            bail!("Codex stdin backlog exceeds 16 MiB");
        }
        if self
            .sender
            .try_send((line, deadline, permission, reply))
            .is_err()
        {
            self.bytes.fetch_sub(length, Ordering::SeqCst);
            state.close("Codex stdin queue is full or closed", true);
            bail!("Codex stdin queue is full or closed");
        }
        Ok(receiver)
    }
}

/// Stop a blocked pipe at its deadline even when a read-only RPC has already retired.
pub(crate) fn wait(
    receiver: mpsc::Receiver<Result<()>>,
    deadline: Instant,
    state: &ClientState,
) -> Result<()> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    match receiver.recv_timeout(remaining) {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(error)) => Err(error),
        Err(_) => {
            state.close(
                "Codex stdin deadline expired; request may already have been accepted",
                true,
            );
            bail!("Codex stdin deadline expired; request may already have been accepted");
        }
    }
}

#[cfg(test)]
#[path = "tests/protocol_writer_guard_tests.rs"]
mod guard_tests;
