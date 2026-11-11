//! Gives controller stdin one bounded writer thread so a blocked pipe cannot hold the UI.

use std::fmt;
use std::io::Write;
use std::process::ChildStdin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use anyhow::{bail, Result};
use serde_json::Value;

use super::transport::{trace_frame, Trace};

const WRITE_TIMEOUT: Duration = Duration::from_millis(500);
const MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;
type WriteRequest = (String, mpsc::Sender<std::result::Result<(), String>>);

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
    sender: mpsc::SyncSender<WriteRequest>,
}

impl InputWriter {
    /// Start one bounded writer that owns the helper stdin handle.
    pub(super) fn start(mut stdin: ChildStdin, trace: Trace, stopped: Arc<AtomicBool>) -> Self {
        let (sender, receiver) = mpsc::sync_channel::<WriteRequest>(8);
        thread::spawn(move || {
            while let Ok((line, reply)) = receiver.recv() {
                let result = if stopped.load(Ordering::SeqCst) {
                    Err("Claude controller is closed".to_string())
                } else {
                    if let Ok(frame) = serde_json::from_str::<Value>(&line) {
                        trace_frame(&trace, "client", &frame);
                    }
                    stdin
                        .write_all(line.as_bytes())
                        .and_then(|_| stdin.flush())
                        .map_err(|error| format!("write Claude controller stdin: {error}"))
                };
                let _ = reply.send(result);
            }
        });
        Self { sender }
    }

    /// Wait briefly for one complete frame to reach controller stdin.
    pub(super) fn write(&self, frame: &Value) -> Result<()> {
        let line = format!("{frame}\n");
        if line.len() > MAX_FRAME_BYTES {
            return Err(OversizedFrame.into());
        }
        let (reply, receiver) = mpsc::channel();
        if self.sender.try_send((line, reply)).is_err() {
            bail!("Claude controller stdin queue is full or closed");
        }
        match receiver.recv_timeout(WRITE_TIMEOUT) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => bail!(error),
            Err(_) => bail!("Claude controller stdin did not finish within 500 ms; request may already have been accepted"),
        }
    }
}
