//! Transfer native ownership once, starting label work only for real daemon launches.

use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use super::mailbox::Mailbox;
use super::owner::{Channels, Owner};
use super::{EngineHandle, Shared};
use crate::backend::{BackendClient, EventReceiver};
use crate::session::state::SessionState;

impl EngineHandle {
    /// Transfer a backend without opening a label cache for direct callers and isolated owner tests.
    pub fn spawn(
        session: SessionState,
        client: Box<dyn BackendClient>,
        events: EventReceiver,
    ) -> (Self, mpsc::Receiver<()>) {
        Self::spawn_inner(session, client, events, false)
    }

    /// Start the existing label service with the daemon owner before native history can arrive.
    pub(crate) fn spawn_with_tool_labels(
        session: SessionState,
        client: Box<dyn BackendClient>,
        events: EventReceiver,
    ) -> (Self, mpsc::Receiver<()>) {
        Self::spawn_inner(session, client, events, true)
    }

    fn spawn_inner(
        session: SessionState,
        client: Box<dyn BackendClient>,
        events: EventReceiver,
        labels: bool,
    ) -> (Self, mpsc::Receiver<()>) {
        let (ordinary, ordinary_rx) = Mailbox::new(64, 8 * 1024 * 1024);
        let (control, control_rx) = Mailbox::new(16, 2 * 1024 * 1024);
        let (reads, reads_rx) = Mailbox::new(64, 1024 * 1024);
        let (wake, wake_rx) = mpsc::sync_channel(1);
        let (notice, notice_rx) = mpsc::sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let changed = Arc::new(AtomicBool::new(true));
        let goal_interest = Arc::new(AtomicBool::new(false));
        let task_interest = Arc::new(AtomicBool::new(false));
        let channels = Channels {
            ordinary: ordinary_rx,
            control: control_rx,
            reads: reads_rx,
            wake: wake.clone(),
            wake_rx,
            notice,
            stop: stop.clone(),
            changed: changed.clone(),
            goal_interest: goal_interest.clone(),
            task_interest: task_interest.clone(),
        };
        let mut owner = Owner::new(session, client, events, channels);
        if labels {
            owner.start_tool_labels();
        }
        let thread = thread::spawn(move || owner.run());
        (
            Self(Arc::new(Shared {
                ordinary,
                control,
                reads,
                wake,
                stop,
                changed,
                goal_interest,
                task_interest,
                thread: Mutex::new(Some(thread)),
            })),
            notice_rx,
        )
    }
}
