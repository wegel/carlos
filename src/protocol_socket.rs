//! Attach only to an explicit local endpoint; the shared server outlives this socket.

use std::io::Write;
use std::path::Path;
use std::process::ChildStdout;
use std::sync::Arc;

use anyhow::Result;

use crate::codex_diagnostics::Diagnostics;
use crate::protocol_events::EventQueue;
use crate::protocol_state::ClientState;

#[cfg(unix)]
#[path = "protocol_socket_unix.rs"]
mod unix;

/// Publish the connection before native replay can ask the UI a question.
pub(crate) enum PendingReader {
    Json(ChildStdout, Diagnostics),
    #[cfg(unix)]
    Socket(unix::SocketReader),
}

/// Keep the shared shutdown handle independent of readers and potentially blocked writes.
pub(crate) struct SocketParts {
    pub(crate) state: Arc<ClientState>,
    pub(crate) input: Box<dyn Write + Send>,
    pub(crate) reader: PendingReader,
}

/// Follow the official client's local WebSocket endpoint without changing auth or server settings.
pub(crate) fn prepare(path: &Path, events: EventQueue) -> Result<SocketParts> {
    #[cfg(unix)]
    {
        unix::prepare(path, events)
    }
    #[cfg(not(unix))]
    {
        let _ = (path, events);
        anyhow::bail!(
            "--codex-socket currently requires a Unix platform; owned Codex mode remains available"
        )
    }
}

impl PendingReader {
    /// Start the selected reader only after its connection handle can answer callbacks.
    pub(crate) fn start(self, state: Arc<ClientState>) {
        match self {
            Self::Json(stdout, diagnostics) => {
                crate::protocol_reader::spawn(stdout, state, diagnostics)
            }
            #[cfg(unix)]
            Self::Socket(reader) => reader.start(state),
        }
    }
}
