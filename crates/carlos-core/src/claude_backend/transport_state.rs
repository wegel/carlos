//! Separates a closed stdout pipe from a requested and completed child shutdown.

use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::owned_child::OwnedChild;

use super::transport::{reject_pending, Pending};
use super::transport_workers::TransportWorkers;

pub(super) const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(3);
pub(super) const SHUTDOWN_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Chooses whether a failed helper needs a signal before the bounded grace.
pub(super) enum ShutdownRequest {
    Graceful(Instant),
    Terminate,
}

#[derive(Default)]
enum StdoutState {
    #[default]
    Open,
    Closed,
}

#[derive(Default, PartialEq)]
enum ShutdownState {
    #[default]
    Running,
    Requested,
    Reaped,
}

#[derive(Default)]
struct State {
    stdout: StdoutState,
    shutdown: ShutdownState,
}

/// Rejects calls after EOF while allowing exactly one later child-shutdown request.
#[derive(Default)]
pub(super) struct TransportState {
    state: Mutex<State>,
    changed: Condvar,
    #[cfg(any(test, feature = "test-support"))]
    pub(super) shutdown_requests: std::sync::atomic::AtomicUsize,
}

impl TransportState {
    /// Test whether either EOF or a shutdown request has closed the request path.
    pub(super) fn is_closed(&self) -> bool {
        let state = self.state.lock().expect("transport state");
        matches!(state.stdout, StdoutState::Closed) || state.shutdown != ShutdownState::Running
    }

    /// Test whether a failure has already claimed the bounded child shutdown.
    pub(super) fn shutdown_requested(&self) -> bool {
        self.state.lock().expect("transport state").shutdown != ShutdownState::Running
    }

    /// Close only the request path; EOF does not prove that the child has exited.
    pub(super) fn stdout_closed(&self, pending: &Pending) {
        self.state.lock().expect("transport state").stdout = StdoutState::Closed;
        reject_pending(pending);
    }

    /// Claim child shutdown once, reject calls now, and reap within the bounded grace.
    pub(super) fn request_shutdown(
        self: &Arc<Self>,
        child: Arc<Mutex<OwnedChild>>,
        pending: &Pending,
        workers: &Arc<TransportWorkers>,
        request: ShutdownRequest,
    ) {
        let deadline = match request {
            ShutdownRequest::Graceful(deadline) => deadline,
            ShutdownRequest::Terminate => Instant::now() + SHUTDOWN_TIMEOUT,
        };
        let mut state = self.state.lock().expect("transport state");
        if state.shutdown != ShutdownState::Running {
            return;
        }
        state.shutdown = ShutdownState::Requested;
        #[cfg(any(test, feature = "test-support"))]
        self.shutdown_requests
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        drop(state);
        reject_pending(pending);
        if matches!(request, ShutdownRequest::Terminate) {
            if let Ok(mut child) = child.lock() {
                if matches!(child.try_wait(), Ok(None)) {
                    child.terminate();
                }
            }
        }
        let owner = self.clone();
        workers.spawn(move || {
            reap_child(&child, deadline);
            owner.state.lock().expect("transport state").shutdown = ShutdownState::Reaped;
            owner.changed.notify_all();
        });
    }

    /// Wait only within the original grace, then use main's kill-and-reap fallback.
    pub(super) fn wait_for_shutdown(&self, child: &Mutex<OwnedChild>, deadline: Instant) {
        let (state, _) = self
            .changed
            .wait_timeout_while(
                self.state.lock().expect("transport state"),
                deadline.saturating_duration_since(Instant::now()),
                |state| state.shutdown != ShutdownState::Reaped,
            )
            .expect("transport state");
        if state.shutdown == ShutdownState::Reaped {
            return;
        }
        drop(state);
        // A delayed reaper must not extend the grace. Child::wait after kill matches main.
        reap_child(child, deadline);
        self.state.lock().expect("transport state").shutdown = ShutdownState::Reaped;
        self.changed.notify_all();
    }

    /// Bound regression-test waits even if a shutdown request fails to start its worker.
    #[cfg(test)]
    pub(super) fn wait_for_shutdown_timeout(&self, timeout: Duration) -> bool {
        let (state, _) = self
            .changed
            .wait_timeout_while(
                self.state.lock().expect("transport state"),
                timeout,
                |state| state.shutdown != ShutdownState::Reaped,
            )
            .expect("transport state");
        state.shutdown == ShutdownState::Reaped
    }
}

fn reap_child(child: &Mutex<OwnedChild>, deadline: Instant) {
    loop {
        {
            let Ok(mut child) = child.lock() else {
                return;
            };
            if !matches!(child.try_wait(), Ok(None)) {
                return;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return;
            }
        }
        std::thread::sleep(
            SHUTDOWN_POLL_INTERVAL.min(deadline.saturating_duration_since(Instant::now())),
        );
    }
}
