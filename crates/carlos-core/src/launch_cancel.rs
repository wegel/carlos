//! Stop an owned backend even when its launch worker has not yet returned a session.

use std::fmt;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct State {
    cancelled: bool,
    stop: Option<Arc<dyn Fn() + Send + Sync>>,
}

/// One pending launch keeps a cancellation hook as soon as it owns a native process.
#[derive(Clone, Default)]
pub struct LaunchCancellation(Arc<Mutex<State>>);

/// A cancelled launch must not publish a ready session or start another process.
#[derive(Debug)]
pub struct LaunchCancelled;

impl LaunchCancellation {
    /// Signal the current captured process without waiting for its RPC reply or joining its owner.
    pub fn cancel(&self) {
        let stop = {
            let mut state = self.0.lock().expect("launch cancellation");
            state.cancelled = true;
            state.stop.take()
        };
        if let Some(stop) = stop {
            stop();
        }
    }

    /// Reject work before spawning and before publishing a completed launch.
    pub fn check(&self) -> Result<(), LaunchCancelled> {
        if self.0.lock().expect("launch cancellation").cancelled {
            Err(LaunchCancelled)
        } else {
            Ok(())
        }
    }

    pub(crate) fn register(
        &self,
        stop: impl Fn() + Send + Sync + 'static,
    ) -> Result<(), LaunchCancelled> {
        let stop = Arc::new(stop);
        let cancelled = {
            let mut state = self.0.lock().expect("launch cancellation");
            if state.cancelled {
                true
            } else {
                state.stop = Some(stop.clone());
                false
            }
        };
        if cancelled {
            stop();
            Err(LaunchCancelled)
        } else {
            Ok(())
        }
    }
}

impl fmt::Display for LaunchCancelled {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("backend launch cancelled")
    }
}
impl std::error::Error for LaunchCancelled {}
