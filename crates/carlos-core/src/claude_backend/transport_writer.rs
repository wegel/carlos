//! Gives controller stdin one bounded writer thread so a blocked pipe cannot hold the UI.

use std::fmt;
use std::io::Write;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use anyhow::{bail, Result};
use serde_json::Value;

use crate::dispatch_guard::DispatchGuard;
use crate::write_timing::{Stage, WriteTiming};

use super::transport::{trace_frame, Trace};
use super::transport_state::TransportState;
use super::transport_workers::TransportWorkers;

const WRITE_TIMEOUT: Duration = Duration::from_millis(500);
const MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;
type WriteRequest = (
    String,
    DispatchGuard,
    mpsc::Sender<Result<()>>,
    Arc<WriteTiming>,
);
type PendingWrite = (mpsc::Receiver<Result<()>>, Arc<WriteTiming>);

#[derive(Debug)]
/// Rejects an input frame before any bytes reach the helper.
pub(super) struct OversizedFrame;

impl fmt::Display for OversizedFrame {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Claude controller request exceeds 4 MiB")
    }
}

impl std::error::Error for OversizedFrame {}

/// Owns the only thread allowed to write controller stdin.
pub(super) struct InputWriter {
    sender: Mutex<Option<mpsc::SyncSender<WriteRequest>>>,
}

impl InputWriter {
    /// Start one bounded writer that owns the helper stdin handle.
    pub(super) fn start(
        mut stdin: impl Write + Send + 'static,
        trace: Trace,
        state: Arc<TransportState>,
        workers: Arc<TransportWorkers>,
    ) -> Self {
        let (sender, receiver) = mpsc::sync_channel::<WriteRequest>(8);
        workers.spawn(move || {
            while let Ok((line, permission, reply, timing)) = receiver.recv() {
                timing.mark(Stage::Dequeued);
                let result = if let Err(error) = permission.check() {
                    Err(anyhow::Error::new(error))
                } else if state.is_closed() {
                    Err(anyhow::anyhow!("Claude controller is closed"))
                } else {
                    timing.mark(Stage::Checked);
                    if trace.is_some() {
                        if let Ok(frame) = serde_json::from_str::<Value>(&line) {
                            trace_frame(&trace, "client", &frame);
                        }
                    }
                    timing.mark(Stage::Ready);
                    let result = stdin
                        .write_all(line.as_bytes())
                        .and_then(|_| stdin.flush())
                        .map_err(|error| anyhow::anyhow!("write Claude controller stdin: {error}"));
                    timing.mark(Stage::Written);
                    result
                };
                let _ = reply.send(result);
            }
        });
        Self {
            sender: Mutex::new(Some(sender)),
        }
    }

    /// Wait briefly for one complete frame to reach controller stdin.
    pub(super) fn write_guarded(&self, frame: &Value, permission: DispatchGuard) -> Result<()> {
        let (receiver, timing) = self.enqueue_timed(frame, permission)?;
        match receiver.recv_timeout(WRITE_TIMEOUT) {
            Ok(result) => result,
            Err(_) => bail!("Claude controller stdin did not finish within 500 ms; request may already have been accepted ({timing}; ready is after trace)"),
        }
    }

    #[cfg(test)]
    pub(super) fn enqueue(
        &self,
        frame: &Value,
        permission: DispatchGuard,
    ) -> Result<mpsc::Receiver<Result<()>>> {
        self.enqueue_timed(frame, permission)
            .map(|(receiver, _)| receiver)
    }

    fn enqueue_timed(&self, frame: &Value, permission: DispatchGuard) -> Result<PendingWrite> {
        permission.check()?;
        let line = format!("{frame}\n");
        if line.len() > MAX_FRAME_BYTES {
            return Err(OversizedFrame.into());
        }
        let (reply, receiver) = mpsc::channel();
        let timing = Arc::new(WriteTiming::new());
        {
            let sender = self.sender.lock().expect("controller writer");
            let Some(sender) = sender.as_ref() else {
                bail!("Claude controller stdin queue is full or closed");
            };
            if sender
                .try_send((line, permission, reply, timing.clone()))
                .is_err()
            {
                bail!("Claude controller stdin queue is full or closed");
            }
        }
        Ok((receiver, timing))
    }

    /// Disconnect the queue; a detached writer exits when its pipe write returns.
    pub(super) fn close(&self) {
        self.sender.lock().expect("controller writer").take();
    }
}

#[cfg(test)]
#[path = "writer_guard_tests.rs"]
mod guard_tests;
