//! Retain only a bounded native stderr tail to explain a failed local connection.

use std::io::Read;
use std::process::ChildStderr;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

type SharedTail = (Mutex<(Vec<u8>, bool)>, Condvar);

#[derive(Clone)]
/// Drain stderr independently so diagnostics cannot block native stdout or the UI.
pub(crate) struct Diagnostics(Arc<SharedTail>);

/// Start a reader that retains at most eight KiB, regardless of native output volume.
pub(crate) fn capture(stderr: Option<ChildStderr>) -> Diagnostics {
    let diagnostics = Diagnostics(Arc::new((Mutex::new((Vec::new(), false)), Condvar::new())));
    let shared = diagnostics.clone();
    thread::spawn(move || {
        if let Some(mut stderr) = stderr {
            let mut buffer = [0; 2048];
            while let Ok(length) = stderr.read(&mut buffer) {
                if length == 0 {
                    break;
                }
                let mut state = shared.0 .0.lock().expect("Codex diagnostics poisoned");
                state.0.extend_from_slice(&buffer[..length]);
                let excess = state.0.len().saturating_sub(8192);
                state.0.drain(..excess);
            }
        }
        shared.0 .0.lock().expect("Codex diagnostics poisoned").1 = true;
        shared.0 .1.notify_all();
    });
    diagnostics
}

impl Diagnostics {
    /// Give an exited proxy's stderr a bounded chance to finish before reporting EOF.
    pub(crate) fn explain(&self, message: &str) -> String {
        let state = self.0 .0.lock().expect("Codex diagnostics poisoned");
        let (state, _) = self
            .0
             .1
            .wait_timeout_while(state, Duration::from_millis(50), |state| !state.1)
            .expect("Codex diagnostics poisoned");
        let tail = String::from_utf8_lossy(&state.0);
        let tail = tail.trim();
        if tail.is_empty() {
            message.into()
        } else {
            format!("{message}: {tail}")
        }
    }
}
